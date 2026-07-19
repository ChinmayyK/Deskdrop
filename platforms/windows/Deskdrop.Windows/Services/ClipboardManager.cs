using System;
using System.Collections.Generic;
using System.Drawing;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Forms;

namespace Deskdrop.Windows
{
// ── History Item ─────────────────────────────────────────────────────────

    public class HistoryItem
    {
        public string Id { get; set; } = Guid.NewGuid().ToString();
        public bool IsPinned { get; set; } = false;
        public string PinColor => IsPinned ? "#32ADE6" : "#8E8E93";
        public string TypeIcon { get; set; } = "📝";
        public string Summary { get; set; } = "";
        public string FullText { get; set; } = "";
        public string Source { get; set; } = "";
        public string RelativeTime { get; set; } = "Just now";
        public DateTime Time { get; set; } = DateTime.Now;
    }

    // ── Clipboard Manager ────────────────────────────────────────────────────

    public sealed class ClipboardManager : IDisposable
    {
        private IntPtr _handle;
        private System.Threading.Timer? _pollTimer;
        private System.Threading.Timer? _watchTimer;
        private uint _lastSequenceNumber;

        // Thread-safe suppress counter: incremented before we write to the clipboard
        // programmatically so the watcher skips that change and doesn't re-push it.
        private int _suppressCount;

        // Track connected peer names for status and icon state.
        private readonly HashSet<string> _connectedPeers =
            new(StringComparer.OrdinalIgnoreCase);

        // In-memory history (max 100 items, newest first).
        private readonly List<HistoryItem> _history = new();
        private readonly object _histLock = new();

        // ── Events ────────────────────────────────────────────────────────────

        public event Action<string>?       StatusChanged;           // status line text
        public event Action<string,string,string>? TofuPromptRequested;    // (id, name, fingerprint)
        public event Action<string,string>? ClipboardReceived;      // (text, fromDevice)
        public event Action<HistoryItem>?  HistoryItemAdded;
        public event Action<string?>?      QuickContextUpdated;     // (text or null)
        public event Action<string, string, string>? IncomingCallRequested;   // (callerName, deviceId, state)
        public event Action<string>?       SystemHealthUpdated;     // json health payload

        private string? _quickContextText;
        public string? QuickContextText => _quickContextText;

        // ── Lifecycle ─────────────────────────────────────────────────────────

        public void Start(string? deviceName = null, ushort port = 0)
        {
            _handle = NativeCore.deskdrop_start(deviceName, port);
            if (_handle == IntPtr.Zero)
            {
                StatusChanged?.Invoke(
                    "❌ Engine failed to start — deskdrop_core.dll missing or incompatible");
                return;
            }

            // Push persisted settings to engine immediately
            System.Threading.Tasks.Task.Run(() =>
            {
                using var key = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(@"Software\Deskdrop");
                if (key != null)
                {
                    DaemonClient.Send(new
                    {
                        cmd = "save_settings",
                        sync_enabled = (int?)key.GetValue("SyncEnabled", 1) == 1,
                        sync_text = (int?)key.GetValue("SyncText", 1) == 1,
                        sync_images = (int?)key.GetValue("SyncImages", 1) == 1,
                        sync_files = (int?)key.GetValue("SyncFiles", 1) == 1,
                        require_tofu_confirmation = (int?)key.GetValue("RequireTofu", 1) == 1,
                        show_receive_notification = (int?)key.GetValue("ShowNotifications", 1) == 1,
                    });
                }
            });

            RefreshStatus();
            _pollTimer  = new System.Threading.Timer(_ => DrainEvents(),    null, 0,   20);
            _watchTimer = new System.Threading.Timer(_ => CheckClipboard(), null, 200, 100);
            _lastSequenceNumber = GetClipboardSequenceNumber();
        }

        public void Stop()
        {
            _pollTimer?.Dispose();  _pollTimer  = null;
            _watchTimer?.Dispose(); _watchTimer = null;
            if (_handle != IntPtr.Zero) { NativeCore.deskdrop_stop(_handle); _handle = IntPtr.Zero; }
        }

        public void RestartDaemon()
        {
            Stop();
            System.Threading.Thread.Sleep(500);
            Start();
        }

        public void Dispose() => Stop();

        /// Call after the user responds Yes/No to a TOFU dialog.
        public void RespondToTrust(string deviceId, bool trust)
        {
            if (_handle != IntPtr.Zero)
                NativeCore.deskdrop_trust_peer(_handle, deviceId, trust ? 1 : 0);
            RefreshStatus();
        }

        public List<HistoryItem> GetHistory()
        {
            lock (_histLock)
            {
                return _history.OrderByDescending(x => x.IsPinned).ThenByDescending(x => x.Time).ToList();
            }
        }

        public void DeleteHistory(string id)
        {
            lock (_histLock)
            {
                _history.RemoveAll(x => x.Id == id);
            }
            System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                var item = DeskdropStore.Shared.History.FirstOrDefault(x => x.Id == id);
                if (item != null) DeskdropStore.Shared.History.Remove(item);
            });
        }

        public void TogglePinHistory(string id)
        {
            lock (_histLock)
            {
                var item = _history.FirstOrDefault(x => x.Id == id);
                if (item != null)
                {
                    item.IsPinned = !item.IsPinned;
                }
            }
            System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                var item = DeskdropStore.Shared.History.FirstOrDefault(x => x.Id == id);
                if (item != null) item.IsPinned = !item.IsPinned;
            });
        }

        // ── Outgoing: watch local clipboard ──────────────────────────────────

        private void CheckClipboard()
        {
            if (_handle == IntPtr.Zero) return;
            uint seq = GetClipboardSequenceNumber();
            if (seq == _lastSequenceNumber) return;
            _lastSequenceNumber = seq;

            // Consume one suppress token; if we're in suppress mode, skip.
            if (Interlocked.Decrement(ref _suppressCount) >= 0) return;
            Interlocked.Exchange(ref _suppressCount, 0); // clamp below zero → 0

            var thread = new Thread(PushLocalClipboard);
            thread.SetApartmentState(ApartmentState.STA);
            thread.IsBackground = true;
            thread.Start();

            // Sync Win+V history
            Task.Run(SyncWinVHistory);
        }

        private async Task SyncWinVHistory()
        {
            try
            {
                var history = await global::Windows.ApplicationModel.DataTransfer.Clipboard.GetHistoryItemsAsync();
                if (history.Status == global::Windows.ApplicationModel.DataTransfer.ClipboardHistoryItemsResultStatus.Success)
                {
                    foreach (var item in history.Items)
                    {
                        if (item.Content.Contains(global::Windows.ApplicationModel.DataTransfer.StandardDataFormats.Text))
                        {
                            var text = await item.Content.GetTextAsync();
                            if (!string.IsNullOrEmpty(text))
                            {
                                bool exists = false;
                                lock (_histLock)
                                {
                                    exists = _history.Any(h => h.FullText == text);
                                }
                                if (!exists)
                                {
                                    AddHistory(new HistoryItem
                                    {
                                        Summary = text.Length > 80 ? text[..77] + "…" : text,
                                        FullText = text, Source = "Win+V",
                                        Time = item.Timestamp.DateTime, TypeIcon = "📄",
                                    });
                                }
                            }
                        }
                    }
                }
            }
            catch { /* Ignore if UWP APIs fail or history is disabled */ }
        }

        public void PushLocalClipboard()
        {
            if (_handle == IntPtr.Zero) return;
            NativeCore.SetThreadExecutionState(NativeCore.ES_CONTINUOUS | NativeCore.ES_SYSTEM_REQUIRED);
            try
            {
                if (Clipboard.ContainsData("ExcludeClipboardContentFromMonitorProcessing") || 
                    Clipboard.ContainsData("Clipboard Viewer Ignore")) 
                    return;

                if (Clipboard.ContainsText())
                {
                    var text = Clipboard.GetText();
                    if (string.IsNullOrEmpty(text)) return;
                    
                    _quickContextText = text;
                    QuickContextUpdated?.Invoke(text);
                    
                    NativeCore.deskdrop_push_text(_handle, text);
                    AddHistory(new HistoryItem
                    {
                        Summary  = text.Length > 80 ? text[..77] + "…" : text,
                        FullText = text, Source = "local",
                        Time = DateTime.Now, TypeIcon = "📄",
                    });
                    return;
                }
                if (Clipboard.ContainsImage())
                {
                    using var img = Clipboard.GetImage();
                    if (img == null) return;
                    using var ms = new MemoryStream();
                    img.Save(ms, System.Drawing.Imaging.ImageFormat.Png);
                    var bytes = ms.ToArray();
                    NativeCore.deskdrop_push_image(_handle, "image/png", bytes, (UIntPtr)bytes.Length);
                    AddHistory(new HistoryItem
                    {
                        Summary = $"Image ({bytes.Length / 1024} KB)",
                        Source  = "local", Time = DateTime.Now, TypeIcon = "🖼️",
                    });
                    return;
                }
                if (Clipboard.ContainsFileDropList())
                {
                    var files = Clipboard.GetFileDropList();
                    if (files == null || files.Count == 0) return;
                    
                    foreach (var path in files)
                    {
                        var name  = Path.GetFileName(path);
                        try
                        {
                            // Send via IPC directly using the path to avoid memory spikes on large files
                            DaemonClient.SendFilePath(path, name, "application/octet-stream");
                        }
                        catch
                        {
                            // Fallback if the Daemon doesn't support send_file_path IPC command
                            PushFile(path);
                        }
                        AddHistory(new HistoryItem
                        {
                            Summary = name, Source = "local",
                            Time = DateTime.Now, TypeIcon = "📎",
                        });
                    }
                }
            }
            catch { /* clipboard is inherently racy on Windows */ }
            finally
            {
                NativeCore.SetThreadExecutionState(NativeCore.ES_CONTINUOUS);
            }
        }

        public void PushText(string text)
        {
            if (_handle == IntPtr.Zero || string.IsNullOrEmpty(text)) return;
            NativeCore.SetThreadExecutionState(NativeCore.ES_CONTINUOUS | NativeCore.ES_SYSTEM_REQUIRED);
            try
            {
                NativeCore.deskdrop_push_text(_handle, text);
                AddHistory(new HistoryItem
                {
                    Summary = text.Length > 80 ? text[..77] + "…" : text,
                    FullText = text, Source = "local",
                    Time = DateTime.Now, TypeIcon = "📋",
                });
            }
            finally
            {
                NativeCore.SetThreadExecutionState(NativeCore.ES_CONTINUOUS);
            }
        }

        public void PushFile(string path, string? targetDevice = null)
        {
            if (_handle == IntPtr.Zero || !File.Exists(path)) return;
            NativeCore.SetThreadExecutionState(NativeCore.ES_CONTINUOUS | NativeCore.ES_SYSTEM_REQUIRED);
            try
            {
                var name = Path.GetFileName(path);
                try
                {
                    NativeCore.deskdrop_send_file_path(_handle, targetDevice, path, name, "application/octet-stream");
                }
                catch (EntryPointNotFoundException)
                {
                    // Fallback to older deskdrop_push_file which loads the file into memory
                    byte[] data = File.ReadAllBytes(path);
                    NativeCore.deskdrop_push_file(_handle, name, data, (UIntPtr)data.Length);
                }
                AddHistory(new HistoryItem
                {
                    Summary = name, Source = "local",
                    Time = DateTime.Now, TypeIcon = "📎",
                });
            }
            finally
            {
                NativeCore.SetThreadExecutionState(NativeCore.ES_CONTINUOUS);
            }
        }

        public void PushCameraFrame(byte[] jpegBytes)
        {
            if (_handle == IntPtr.Zero) return;
            NativeCore.SetThreadExecutionState(NativeCore.ES_CONTINUOUS | NativeCore.ES_SYSTEM_REQUIRED);
            try
            {
                NativeCore.deskdrop_push_video_frame(_handle, jpegBytes, (UIntPtr)jpegBytes.Length);
            }
            finally
            {
                NativeCore.SetThreadExecutionState(NativeCore.ES_CONTINUOUS);
            }
        }

        // ── Incoming: drain Rust event queue ─────────────────────────────────

        private void DrainEvents()
        {
            if (_handle == IntPtr.Zero) return;
            while (true)
            {
                var ev = NativeCore.deskdrop_poll_event(_handle);
                if (ev == IntPtr.Zero) break;
                try   { HandleEvent(ev); }
                finally { NativeCore.deskdrop_free_event(ev); }
            }
        }

        private void HandleEvent(IntPtr ev)
        {
            int kind = NativeCore.deskdrop_event_type(ev);
            switch (kind)
            {
                // Text auto-applied (engine decided to apply it immediately).
                case NativeCore.PB_EVENT_CLIPBOARD_TEXT:
                {
                    var text = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_text(ev));
                    var from = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_name(ev)) ?? "Unknown";
                    if (text != null) ApplyText(text, from);
                    break;
                }

                // Text available (timeline-first): notify user, don't auto-apply.
                case NativeCore.PB_EVENT_CLIPBOARD_AVAILABLE:
                {
                    var text = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_text(ev));
                    var from = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_name(ev)) ?? "Unknown";
                    if (text != null)
                    {
                        string preview = text.Length > 80 ? text[..77] + "…" : text;
                        AddHistory(new HistoryItem
                        {
                            Summary = preview, FullText = text, Source = from,
                            Time = DateTime.Now, TypeIcon = "📋",
                        });
                        ClipboardReceived?.Invoke(text, from);
                        StatusChanged?.Invoke($"📋 Clipboard from {from}");
                    }
                    break;
                }

                case NativeCore.PB_EVENT_CLIPBOARD_IMAGE:
                {
                    var path = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_text(ev));
                    var from = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_name(ev)) ?? "Unknown";
                    if (path != null)
                    {
                        AddHistory(new HistoryItem
                        {
                            Summary = $"Image from {from}", FullText = path, Source = from,
                            Time = DateTime.Now, TypeIcon = "🖼️",
                        });
                        StatusChanged?.Invoke($"🖼️ Image received from {from}");
                        System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                            NotificationHelper.ShowToast($"Image from {from}", "Saved to Downloads");
                        });
                    }
                    break;
                }

                case NativeCore.PB_EVENT_CLIPBOARD_FILE:
                {
                    var path = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_text(ev));
                    var from = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_name(ev)) ?? "Unknown";
                    if (path != null)
                    {
                        var name = System.IO.Path.GetFileName(path);
                        AddHistory(new HistoryItem
                        {
                            Summary = name, FullText = path, Source = from,
                            Time = DateTime.Now, TypeIcon = "📎",
                        });
                        StatusChanged?.Invoke($"📎 File received from {from}");
                        System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                            NotificationHelper.ShowToast($"File from {from}", $"Saved: {name}");
                        });
                    }
                    break;
                }

                case NativeCore.PB_EVENT_FILE_TRANSFER_INCOMING:
                {
                    var tid = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_transfer_id(ev));
                    var name = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_transfer_file_name(ev)) ?? "Unknown File";
                    var from = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_name(ev)) ?? "Unknown";
                    
                    if (tid != null)
                    {
                        StatusChanged?.Invoke($"⬇️ Incoming {name} from {from}...");
                        System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                            new IncomingFileBannerWindow(name, from).Show();
                        });
                        // Do not auto-accept here; let core policy or user UI handle it
                    }
                    break;
                }

                case NativeCore.PB_EVENT_FILE_TRANSFER_COMPLETE:
                {
                    var path = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_transfer_dest_path(ev));
                    var from = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_name(ev)) ?? "Unknown";
                    var name = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_transfer_file_name(ev)) ?? "File";
                    
                    if (path != null)
                    {
                        AddHistory(new HistoryItem
                        {
                            Summary = name, FullText = path, Source = from,
                            Time = DateTime.Now, TypeIcon = "📎",
                        });
                        StatusChanged?.Invoke($"✅ File transfer complete from {from}");
                        System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                            NotificationHelper.ShowToast($"File from {from}", $"Saved: {name}");
                        });
                    }
                    break;
                }

                case NativeCore.PB_EVENT_PAIRING_REQUESTED:
                {
                    var name = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_name(ev)) ?? "Unknown";
                    string id;
                    try { id = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_id(ev)) ?? name; } catch (EntryPointNotFoundException) { id = name; }
                    var fp   = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_fingerprint(ev)) ?? "";
                    TofuPromptRequested?.Invoke(id, name, fp);
                    break;
                }

                case NativeCore.PB_EVENT_PEER_CONNECTED:
                {
                    var peer = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_name(ev)) ?? "Unknown";
                    lock (_connectedPeers) _connectedPeers.Add(peer);
                    RefreshStatus();
                    break;
                }

                case NativeCore.PB_EVENT_PEER_DISCONNECTED:
                {
                    var peer = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_name(ev));
                    lock (_connectedPeers)
                    {
                        if (peer != null) _connectedPeers.Remove(peer);
                        else              _connectedPeers.Clear();
                    }
                    RefreshStatus();
                    break;
                }

                case NativeCore.PB_EVENT_WARNING:
                {
                    var msg = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_text(ev));
                    if (msg != null) StatusChanged?.Invoke($"⚠️ {msg}");
                    break;
                }

                case NativeCore.PB_EVENT_CALL_STATE_CHANGED:
                {
                    var caller = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_name(ev)) ?? "Unknown";
                    string deviceId;
                    try { deviceId = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_id(ev)) ?? caller; } catch (EntryPointNotFoundException) { deviceId = caller; }
                    var state = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_text(ev)) ?? "idle";
                    IncomingCallRequested?.Invoke(caller, deviceId, state);
                    break;
                }

                case NativeCore.PB_EVENT_ACTIVITY_UPDATED:
                {
                    var json = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_text(ev));
                    if (json != null)
                    {
                        try
                        {
                            var activity = System.Text.Json.JsonSerializer.Deserialize<System.Text.Json.JsonElement>(json);
                            if (activity.TryGetProperty("kind", out var kindElem) && kindElem.GetString() == "remote_notification")
                            {
                                string title = activity.TryGetProperty("notification_title", out var t) ? t.GetString() ?? "Notification" : "Notification";
                                string body = activity.TryGetProperty("notification_body", out var b) ? b.GetString() ?? "" : "";
                                string appName = activity.TryGetProperty("app_name", out var a) ? a.GetString() ?? "" : "";
                                
                                string source = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_name(ev)) ?? "Phone";
                                
                                System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                                    NotificationHelper.ShowToast($"{appName} via {source}", $"{title}\n{body}");
                                });
                            }
                        }
                        catch { /* ignore invalid JSON */ }
                    }
                    break;
                }

                case NativeCore.PB_EVENT_SYSTEM_HEALTH_UPDATED:
                {
                    var healthJson = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_text(ev));
                    if (healthJson != null)
                    {
                        SystemHealthUpdated?.Invoke(healthJson);
                    }
                    break;
                }
            }
        }

        public void SendCallAction(string action, string deviceId)
        {
            if (_handle != IntPtr.Zero)
            {
                try { NativeCore.deskdrop_send_call_action(_handle, action, deviceId); } catch (EntryPointNotFoundException) {}
            }
        }

        private void ApplyText(string text, string fromDevice)
        {
            NativeCore.SetThreadExecutionState(NativeCore.ES_CONTINUOUS | NativeCore.ES_SYSTEM_REQUIRED);
            // Suppress watcher: we're writing to the clipboard ourselves.
            Interlocked.Increment(ref _suppressCount);
            var thread = new Thread(() =>
            {
                try   { Clipboard.SetText(text); }
                catch { Interlocked.Decrement(ref _suppressCount); }
            });
            thread.SetApartmentState(ApartmentState.STA);
            thread.IsBackground = true;
            thread.Start();
            thread.Join(300);

            AddHistory(new HistoryItem
            {
                Summary = text.Length > 80 ? text[..77] + "…" : text,
                FullText = text, Source = fromDevice,
                Time = DateTime.Now, TypeIcon = "📋",
            });
            StatusChanged?.Invoke($"📋 Clipboard from {fromDevice}");
            NativeCore.SetThreadExecutionState(NativeCore.ES_CONTINUOUS);
        }

        private void AddHistory(HistoryItem item)
        {
            lock (_histLock)
            {
                _history.RemoveAll(i => i.FullText != null && i.FullText == item.FullText);
                _history.Insert(0, item);
                if (_history.Count > 100) _history.RemoveRange(100, _history.Count - 100);
            }
            
            System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                var storeHist = DeskdropStore.Shared.History;
                var existing = storeHist.FirstOrDefault(x => x.FullText != null && x.FullText == item.FullText);
                if (existing != null) storeHist.Remove(existing);
                storeHist.Insert(0, item);
                if (storeHist.Count > 100) storeHist.RemoveAt(100);
            });

            HistoryItemAdded?.Invoke(item);
        }

        private void RefreshStatus()
        {
            int n;
            lock (_connectedPeers) n = _connectedPeers.Count;
            StatusChanged?.Invoke(_handle == IntPtr.Zero
                ? "⛔ Stopped"
                : n == 0 ? "✅ Running — no devices connected"
                : n == 1 ? "📡 Connected to 1 device"
                : $"📡 Connected to {n} devices");
        }

        public bool IsConnected()
        {
            lock (_connectedPeers) return _connectedPeers.Count > 0;
        }

        [DllImport("user32.dll")]
        private static extern uint GetClipboardSequenceNumber();
    }

    }
