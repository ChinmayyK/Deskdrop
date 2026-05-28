// Deskdrop for Windows
// C# wrapper around the Rust core (P/Invoke).
//
// Build: dotnet publish -c Release -r win-x64 --self-contained false
// The Rust DLL (deskdrop_core.dll) must be in the same directory as the EXE.

using System;
using System.Collections.Generic;
using System.Drawing;
using System.IO;
using System.IO.Pipes;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Forms;
using Microsoft.Win32;

namespace Deskdrop.Windows
{
    // ── P/Invoke declarations ────────────────────────────────────────────────

    internal static class NativeCore
    {
        private const string DLL = "deskdrop_core";

        // Event codes (must match Rust CR_EVENT_* constants)
        public const int PB_EVENT_NONE = 0;
        public const int PB_EVENT_CLIPBOARD_TEXT = 1;
        public const int PB_EVENT_CLIPBOARD_IMAGE = 2;
        public const int PB_EVENT_CLIPBOARD_FILE = 3;
        public const int PB_EVENT_PAIRING_REQUESTED = 4; // TOFU prompt
        public const int PB_EVENT_PEER_CONNECTED = 5;
        public const int PB_EVENT_PEER_DISCONNECTED = 6;
        public const int PB_EVENT_WARNING = 7;
        public const int PB_EVENT_CLIPBOARD_SYNCED = 8;
        public const int PB_EVENT_CLIPBOARD_AVAILABLE = 11; // timeline-first
        public const int PB_EVENT_FILE_TRANSFER_INCOMING = 12;
        public const int PB_EVENT_FILE_TRANSFER_PROGRESS = 13;
        public const int PB_EVENT_FILE_TRANSFER_COMPLETE = 14;
        public const int PB_EVENT_FILE_TRANSFER_FAILED = 15;
        public const int PB_EVENT_ACTIVITY_UPDATED = 16;
        public const int PB_EVENT_CALL_STATE_CHANGED = 17;
        public const int PB_EVENT_CALL_ACTION = 18;
        public const int PB_EVENT_BATTERY_STATE_CHANGED = 19;
        public const int PB_EVENT_FILE_TRANSFER_PAUSED = 20;
        public const int PB_EVENT_FILE_TRANSFER_RESUMED = 21;
        public const int PB_EVENT_CAMERA_STREAM_REQUEST = 22;
        public const int PB_EVENT_CAMERA_STREAM_ACCEPT = 23;
        public const int PB_EVENT_CAMERA_STREAM_STOP = 24;
        public const int PB_EVENT_CAMERA_FRAME = 25;
        public const int PB_EVENT_SYSTEM_HEALTH_UPDATED = 26;

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr deskdrop_start(
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? deviceName, ushort port);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern void deskdrop_stop(IntPtr handle);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int deskdrop_push_text(
            IntPtr handle, [MarshalAs(UnmanagedType.LPUTF8Str)] string text);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int deskdrop_push_image(
            IntPtr handle, [MarshalAs(UnmanagedType.LPUTF8Str)] string mimeType,
            byte[] data, UIntPtr len);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int deskdrop_push_file(
            IntPtr handle, [MarshalAs(UnmanagedType.LPUTF8Str)] string name,
            byte[] data, UIntPtr len);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int deskdrop_push_video_frame(IntPtr handle, byte[] data, UIntPtr size);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int deskdrop_send_file_path(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? targetDevice,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string path,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string fileName,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string mimeType);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int deskdrop_send_call_action(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string action,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string targetDevice);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr deskdrop_poll_event(IntPtr handle);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int deskdrop_event_type(IntPtr ev);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr deskdrop_event_text(IntPtr ev);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr deskdrop_event_device_name(IntPtr ev);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr deskdrop_event_fingerprint(IntPtr ev);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr deskdrop_event_device_id(IntPtr ev);

        /// Respond to a TOFU prompt. trust=1 to accept, trust=0 to reject.
        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int deskdrop_trust_peer(
            IntPtr handle, [MarshalAs(UnmanagedType.LPUTF8Str)] string deviceName, int trust);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern void deskdrop_free_event(IntPtr ev);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int deskdrop_accept_file_transfer(IntPtr handle, [MarshalAs(UnmanagedType.LPUTF8Str)] string transferIdHex);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr deskdrop_event_transfer_id(IntPtr ev);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr deskdrop_event_transfer_file_name(IntPtr ev);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr deskdrop_event_transfer_dest_path(IntPtr ev);

        public static string? PtrToUtf8String(IntPtr ptr)
        {
            if (ptr == IntPtr.Zero) return null;
            int len = 0;
            while (Marshal.ReadByte(ptr, len) != 0) len++;
            var buf = new byte[len];
            Marshal.Copy(ptr, buf, 0, len);
            return Encoding.UTF8.GetString(buf);
        }
    }

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
            NativeMethods.SetThreadExecutionState(NativeMethods.ES_CONTINUOUS | NativeMethods.ES_SYSTEM_REQUIRED);
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
                NativeMethods.SetThreadExecutionState(NativeMethods.ES_CONTINUOUS);
            }
        }

        public void PushText(string text)
        {
            if (_handle == IntPtr.Zero || string.IsNullOrEmpty(text)) return;
            NativeMethods.SetThreadExecutionState(NativeMethods.ES_CONTINUOUS | NativeMethods.ES_SYSTEM_REQUIRED);
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
                NativeMethods.SetThreadExecutionState(NativeMethods.ES_CONTINUOUS);
            }
        }

        public void PushFile(string path, string? targetDevice = null)
        {
            if (_handle == IntPtr.Zero || !File.Exists(path)) return;
            NativeMethods.SetThreadExecutionState(NativeMethods.ES_CONTINUOUS | NativeMethods.ES_SYSTEM_REQUIRED);
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
                NativeMethods.SetThreadExecutionState(NativeMethods.ES_CONTINUOUS);
            }
        }

        public void PushCameraFrame(byte[] jpegBytes)
        {
            if (_handle == IntPtr.Zero) return;
            NativeMethods.SetThreadExecutionState(NativeMethods.ES_CONTINUOUS | NativeMethods.ES_SYSTEM_REQUIRED);
            try
            {
                NativeCore.deskdrop_push_video_frame(_handle, jpegBytes, (UIntPtr)jpegBytes.Length);
            }
            finally
            {
                NativeMethods.SetThreadExecutionState(NativeMethods.ES_CONTINUOUS);
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
                    var id   = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_id(ev)) ?? name;
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
                    var deviceId = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_id(ev)) ?? caller;
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
                NativeCore.deskdrop_send_call_action(_handle, action, deviceId);
            }
        }

        private void ApplyText(string text, string fromDevice)
        {
            NativeMethods.SetThreadExecutionState(NativeMethods.ES_CONTINUOUS | NativeMethods.ES_SYSTEM_REQUIRED);
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
            NativeMethods.SetThreadExecutionState(NativeMethods.ES_CONTINUOUS);
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

    // ── Tray Application ─────────────────────────────────────────────────────

    internal sealed class TrayApp : ApplicationContext
    {
        private readonly NotifyIcon       _tray;
        private readonly ClipboardManager _mgr = new();
        private readonly ContextMenuStrip _menu = new();

        private readonly ToolStripMenuItem _statusItem;
        private readonly ToolStripMenuItem _sendItem;
        private readonly ToolStripMenuItem _sendFileItem;
        private readonly ToolStripMenuItem _syncToggleItem;

        private MainWindow?            _mainWindow;
        private QuickAccessWindow?     _quickAccessWindow;
        private bool                   _syncEnabled  = true;
        private DateTime               _lastBalloonAt = DateTime.MinValue;

        public TrayApp()
        {
            _statusItem = new ToolStripMenuItem("Starting…") { Enabled = false };

            _sendItem = new ToolStripMenuItem("Send Clipboard to Devices") { Enabled = false };
            _sendItem.Click += OnSendClipboard;

            _sendFileItem = new ToolStripMenuItem("Send File to Devices…") { Enabled = false };
            _sendFileItem.Click += OnSendFile;

            _syncToggleItem = new ToolStripMenuItem("Pause Sync");
            _syncToggleItem.Click += OnToggleSync;

            var historyItem = new ToolStripMenuItem("Open Dashboard…");
            historyItem.Click += (_, _) => OpenDashboard();

            var scanItem = new ToolStripMenuItem("Scan for Devices");
            scanItem.Click += OnScanDevices;

            var connectItem = new ToolStripMenuItem("Connect to Device…");
            connectItem.Click += OnManualConnect;

            var prefsItem = new ToolStripMenuItem("Preferences…");
            prefsItem.Click += (_, _) => OpenDashboard();

            var quitItem = new ToolStripMenuItem("Quit Deskdrop");
            quitItem.Click += (_, _) => { 
                _mgr.Stop(); 
                if (System.Windows.Application.Current != null)
                    System.Windows.Application.Current.Shutdown();
                else
                    Application.Exit(); 
            };

            Microsoft.Win32.SystemEvents.PowerModeChanged += OnPowerModeChanged;
            System.Net.NetworkInformation.NetworkChange.NetworkAddressChanged += OnNetworkAddressChanged;

            _menu.Items.AddRange(new ToolStripItem[]
            {
                _statusItem,
                new ToolStripSeparator(),
                _sendItem,
                _sendFileItem,
                historyItem,
                new ToolStripSeparator(),
                _syncToggleItem,
                scanItem,
                connectItem,
                new ToolStripSeparator(),
                prefsItem,
                new ToolStripSeparator(),
                quitItem,
            });

            _tray = new NotifyIcon
            {
                Icon             = BuildTrayIcon(false),
                Text             = "Deskdrop",
                ContextMenuStrip = _menu,
                Visible          = true,
            };
            _tray.DoubleClick += (_, _) => OpenDashboard();
            _tray.MouseClick += (s, e) => {
                if (e.Button == MouseButtons.Left) {
                    OpenQuickAccess();
                }
            };

            // Register Global Hotkeys
            if (LoadSettings().EnableHotkeys)
            {
                GlobalHotKeyManager.Shared.Register(System.Windows.Input.ModifierKeys.Control | System.Windows.Input.ModifierKeys.Shift, System.Windows.Input.Key.V, () => {
                    System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                        OpenQuickAccess();
                    });
                });

                GlobalHotKeyManager.Shared.Register(System.Windows.Input.ModifierKeys.Control, System.Windows.Input.Key.K, () => {
                    System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                        OpenDashboard();
                        if (_mainWindow != null)
                        {
                            // Open Command Palette
                            _mainWindow.ToggleCommandPaletteGlobal();
                        }
                    });
                });

                GlobalHotKeyManager.Shared.Register(System.Windows.Input.ModifierKeys.Control | System.Windows.Input.ModifierKeys.Shift, System.Windows.Input.Key.L, () => {
                    System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                        System.Threading.Tasks.Task.Run(async () => {
                            var url = await BrowserUrlFetcher.GetActiveBrowserUrl();
                            if (!string.IsNullOrEmpty(url))
                            {
                                _mgr.PushText(url);
                                System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                                    NotificationHelper.ShowToast("Deskdrop", $"Pushed URL: {url}");
                                });
                            }
                            else
                            {
                                System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                                    NotificationHelper.ShowToast("Deskdrop", "Could not detect active browser URL.");
                                });
                            }
                        });
                    });
                });

                GlobalHotKeyManager.Shared.Register(System.Windows.Input.ModifierKeys.Control | System.Windows.Input.ModifierKeys.Shift, System.Windows.Input.Key.D, () => {
                    System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                        var dropZone = new DropZoneWindow(_mgr);
                        dropZone.Show();
                        dropZone.Activate();
                    });
                });

                GlobalHotKeyManager.Shared.Register(System.Windows.Input.ModifierKeys.Control | System.Windows.Input.ModifierKeys.Shift, System.Windows.Input.Key.C, () => {
                    System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                        if (System.Windows.Forms.Clipboard.ContainsText() || System.Windows.Forms.Clipboard.ContainsImage() || System.Windows.Forms.Clipboard.ContainsFileDropList())
                        {
                            _mgr.PushLocalClipboard();
                            NotificationHelper.ShowToast("Deskdrop", "Clipboard sent.");
                        }
                    });
                });
            }

            _mgr.StatusChanged       += OnStatusChanged;
            _mgr.TofuPromptRequested += OnTofuPrompt;
            _mgr.HistoryItemAdded    += item => {
                if (!LoadSettings().ShowNotifications) return;
                System.Windows.Application.Current.Dispatcher.Invoke(() =>
                {
                    string title = item.Source == "local" ? "Sent Clipboard" : $"Received from {item.Source}";
                    NotificationHelper.ShowToast(title, $"{item.TypeIcon} {item.Summary}");
                });
            };

            _mgr.IncomingCallRequested += (caller, deviceId, state) => {
                System.Windows.Application.Current?.Dispatcher.Invoke(() =>
                {
                    if (state == "ringing")
                    {
                        var banner = new IncomingCallBannerWindow(caller);
                        banner.CallAccepted += (s, e) => _mgr.SendCallAction("accept", deviceId);
                        banner.CallDeclined += (s, e) => _mgr.SendCallAction("reject", deviceId);
                        banner.Show();
                    }
                    else
                    {
                        foreach (System.Windows.Window window in System.Windows.Application.Current.Windows)
                        {
                            if (window is IncomingCallBannerWindow bannerWindow)
                            {
                                bannerWindow.Close();
                            }
                        }
                    }
                });
            };

            var s = LoadSettings();
            _syncEnabled = s.SyncEnabled;
            _syncToggleItem.Text = _syncEnabled ? "Pause Sync" : "Resume Sync";
            _mgr.Start(
                deviceName: string.IsNullOrWhiteSpace(s.DeviceName) ? Environment.MachineName : s.DeviceName,
                port: s.Port);
        }

        // ── Status ────────────────────────────────────────────────────────────

        private void OnStatusChanged(string msg)
        {
            if (_tray == null) return;
            System.Windows.Application.Current.Dispatcher.Invoke(() =>
            {
                _statusItem.Text = msg.Length > 63 ? msg[..60] + "…" : msg;
                bool connected = _mgr.IsConnected();
                _tray.Icon = BuildTrayIcon(connected);
                _tray.Text = connected ? "Deskdrop — syncing" : "Deskdrop — idle";
                _sendItem.Enabled = connected;
                _sendFileItem.Enabled = connected;
            });
        }

        // ── Incoming clipboard balloon ────────────────────────────────────────

        private void OnClipboardReceived(string text, string from)
        {
            if (!LoadSettings().ShowNotifications) return;
            if ((DateTime.Now - _lastBalloonAt).TotalSeconds < 3) return;
            _lastBalloonAt = DateTime.Now;
            string preview = text.Length > 60 ? text[..57] + "…" : text;
            System.Windows.Application.Current.Dispatcher.Invoke(() =>
                NotificationHelper.ShowToast($"📋 Clipboard from {from}", preview));
        }

        // ── TOFU ─────────────────────────────────────────────────────────────

        private void OnTofuPrompt(string deviceId, string deviceName, string fingerprint)
        {
            System.Windows.Application.Current.Dispatcher.Invoke(() =>
            {
                OpenDashboard();
                if (_mainWindow != null)
                {
                    _mainWindow.ShowTofuPrompt(deviceId, deviceName, fingerprint);
                }
            });
        }

        // ── Menu actions ──────────────────────────────────────────────────────

        private void OnSendClipboard(object? s, EventArgs e)
        {
            Task.Run(() =>
            {
                if (Clipboard.ContainsText() || Clipboard.ContainsImage() || Clipboard.ContainsFileDropList())
                {
                    _mgr.PushLocalClipboard();
                    System.Windows.Application.Current.Dispatcher.Invoke(() =>
                        NotificationHelper.ShowToast("Deskdrop", "Clipboard sent."));
                }
            });
        }

        private void OnSendFile(object? s, EventArgs e)
        {
            var ofd = new System.Windows.Forms.OpenFileDialog { Title = "Select file to send via Deskdrop" };
            if (ofd.ShowDialog() == DialogResult.OK)
            {
                Task.Run(() => {
                    try {
                        _mgr.PushFile(ofd.FileName);
                        System.Windows.Application.Current.Dispatcher.Invoke(() =>
                            NotificationHelper.ShowToast("Deskdrop", $"Sending {Path.GetFileName(ofd.FileName)}..."));
                    } catch (Exception ex) {
                        System.Windows.Application.Current.Dispatcher.Invoke(() =>
                            NotificationHelper.ShowToast("Deskdrop Error", $"Failed to send file: {ex.Message}"));
                    }
                });
            }
        }

        private void OnToggleSync(object? s, EventArgs e)
        {
            _syncEnabled = !_syncEnabled;
            _syncToggleItem.Text = _syncEnabled ? "Pause Sync" : "Resume Sync";
            using var k = Registry.CurrentUser.CreateSubKey(@"Software\Deskdrop");
            k.SetValue("SyncEnabled", _syncEnabled ? 1 : 0, RegistryValueKind.DWord);
            Task.Run(() => DaemonClient.Send(new { cmd = "save_settings", sync_enabled = _syncEnabled }));
            NotificationHelper.ShowToast("Deskdrop",
                _syncEnabled ? "Clipboard sync resumed." : "Clipboard sync paused.");
        }

        private void OnManualConnect(object? s, EventArgs e)
        {
            System.Windows.Application.Current.Dispatcher.Invoke(() =>
            {
                OpenDashboard();
            });
        }

        private void OnScanDevices(object? s, EventArgs e)
        {
            Task.Run(() => DaemonClient.Send(new { cmd = "rescan_peers" }));
            NotificationHelper.ShowToast("Deskdrop", "Scanning for nearby devices…");
        }

        // ── External Actions (from IPC) ─────────────────────────────────────────
        
        public void PushFileExternal(string filePath)
        {
            try {
                _mgr.PushFile(filePath);
                System.Windows.Application.Current.Dispatcher.Invoke(() =>
                    NotificationHelper.ShowToast("Deskdrop", $"Sending {Path.GetFileName(filePath)}..."));
            } catch (Exception ex) {
                System.Windows.Application.Current.Dispatcher.Invoke(() =>
                    NotificationHelper.ShowToast("Deskdrop Error", $"Failed to send file: {ex.Message}"));
            }
        }
        
        public void PushClipboardExternal()
        {
            OnSendClipboard(this, EventArgs.Empty);
        }

        public void OpenSendFileDialog()
        {
            OnSendFile(this, EventArgs.Empty);
        }

        public void RespondToTrustExternal(string deviceId, bool accepted)
        {
            _mgr.RespondToTrust(deviceId, accepted);
            if (accepted)
            {
                System.Windows.Application.Current.Dispatcher.Invoke(() =>
                    NotificationHelper.ShowToast("Deskdrop", "Device trusted successfully."));
            }
        }

        // ── Dashboard panel ─────────────────────────────────────────────────────

        public void OpenDashboard()
        {
            System.Windows.Application.Current?.Dispatcher.Invoke(() =>
            {
                try
                {
                    if (_mainWindow == null)
                    {
                        _mainWindow = new MainWindow(_mgr);
                        _mainWindow.Closed += (_, _) => _mainWindow = null;
                    }
                    
                    _mainWindow.Show();
                    if (_mainWindow.WindowState == System.Windows.WindowState.Minimized)
                    {
                        _mainWindow.WindowState = System.Windows.WindowState.Normal;
                    }
                    _mainWindow.Activate();
                    _mainWindow.Topmost = true;
                    _mainWindow.Topmost = false;
                    _mainWindow.Focus();
                }
                catch (Exception ex)
                {
                    _mainWindow = null;
                    Program.LogError(ex);
                }
            });
        }

        public void OpenQuickAccess()
        {
            if (_quickAccessWindow != null && _quickAccessWindow.IsLoaded)
            {
                _quickAccessWindow.Activate();
                return;
            }

            _quickAccessWindow = new QuickAccessWindow(_mgr);
            _quickAccessWindow.DashboardRequested += (s, e) => OpenDashboard();
            _quickAccessWindow.Show();
            _quickAccessWindow.Activate();
        }

        // ── Settings ──────────────────────────────────────────────────────────

        public record AppSettings(bool SyncEnabled, bool ShowNotifications,
            string DeviceName, ushort Port, bool HasCompletedOnboarding, bool EnableHotkeys);

        public static AppSettings LoadSettings()
        {
            using var key = Registry.CurrentUser.OpenSubKey(@"Software\Deskdrop");
            if (key == null) return new AppSettings(true, true, "", 47823, false, true);
            return new AppSettings(
                SyncEnabled:       ((int?)key.GetValue("SyncEnabled",       1) ?? 1) != 0,
                ShowNotifications: ((int?)key.GetValue("ShowNotifications", 1) ?? 1) != 0,
                DeviceName:        (string?)key.GetValue("DeviceName", "") ?? "",
                Port:              (ushort)Math.Clamp(
                    (int?)key.GetValue("Port", 47823) ?? 47823, 1024, 65535),
                HasCompletedOnboarding: ((int?)key.GetValue("HasCompletedOnboarding", 0) ?? 0) != 0,
                EnableHotkeys:     ((int?)key.GetValue("EnableHotkeys", 1) ?? 1) != 0);
        }
        
        public static void CompleteOnboarding()
        {
            using var key = Registry.CurrentUser.CreateSubKey(@"Software\Deskdrop");
            key.SetValue("HasCompletedOnboarding", 1);
        }

        // ── Tray icon ─────────────────────────────────────────────────────────

        private static Icon BuildTrayIcon(bool connected)
        {
            using var bmp = new Bitmap(16, 16);
            using var g   = Graphics.FromImage(bmp);
            g.Clear(Color.Transparent);
            var color = connected ? Color.LimeGreen : Color.SlateGray;
            using var pen = new Pen(color, 1.5f);
            // Clipboard outline
            g.DrawRectangle(pen, 2, 4, 11, 11);
            // Clip tab
            g.DrawRectangle(pen, 5, 2, 5, 3);
            if (connected)
            {
                // Green checkmark
                g.DrawLine(pen, 4, 10, 6, 13);
                g.DrawLine(pen, 6, 13, 12, 7);
            }
            var hIcon = bmp.GetHicon();
            var icon  = (Icon)Icon.FromHandle(hIcon).Clone();
            NativeMethods.DestroyIcon(hIcon);
            return icon;
        }

        // ── Helpers ───────────────────────────────────────────────────────────

        private static string FormatFingerprint(string raw)
        {
            var clean = raw.Replace(":", "").ToUpperInvariant();
            var pairs = new List<string>();
            for (int i = 0; i + 1 < clean.Length; i += 2)
                pairs.Add(clean.Substring(i, 2));
            var lines = new List<string>();
            for (int i = 0; i < pairs.Count; i += 8)
                lines.Add(string.Join(":", pairs.GetRange(i, Math.Min(8, pairs.Count - i))));
            return string.Join("\n", lines);
        }

        // Removed ShowInputDialog as we're migrating away from WinForms Dialogs

        private void OnPowerModeChanged(object sender, Microsoft.Win32.PowerModeChangedEventArgs e)
        {
            if (e.Mode == Microsoft.Win32.PowerModes.Resume)
            {
                System.Windows.Application.Current?.Dispatcher.InvokeAsync(() => {
                    _mgr.RestartDaemon();
                });
            }
        }

        private void OnNetworkAddressChanged(object? sender, EventArgs e)
        {
            System.Windows.Application.Current?.Dispatcher.InvokeAsync(() => {
                _mgr.RestartDaemon();
            });
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing) 
            { 
                Microsoft.Win32.SystemEvents.PowerModeChanged -= OnPowerModeChanged;
                System.Net.NetworkInformation.NetworkChange.NetworkAddressChanged -= OnNetworkAddressChanged;
                _tray.Dispose(); 
                _mgr.Dispose(); 
                _menu.Dispose(); 
            }
            base.Dispose(disposing);
        }
    }

    // ── Native helpers ────────────────────────────────────────────────────────

    internal static class NativeMethods
    {
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool DestroyIcon(IntPtr hIcon);

        [DllImport("kernel32.dll", CharSet = CharSet.Auto, SetLastError = true)]
        public static extern uint SetThreadExecutionState(uint esFlags);

        public const uint ES_CONTINUOUS = 0x80000000;
        public const uint ES_SYSTEM_REQUIRED = 0x00000001;
    }

    // ── Entry point ───────────────────────────────────────────────────────────

    internal static class Program
    {
        [STAThread]
        static void Main(string[] args)
        {
            try
            {
                using var mutex = new Mutex(true, $"Deskdrop_SingleInstance_v1_{Environment.UserName}", out bool isNew);
                if (!isNew)
                {
                    if (args.Length > 0)
                    {
                        try
                        {
                            using var client = new NamedPipeClientStream(".", $"DeskdropIPC_{Environment.UserName}", PipeDirection.Out);
                            client.Connect(1000);
                            using var writer = new StreamWriter(client);
                            writer.WriteLine(string.Join("|", args));
                            writer.Flush();
                        }
                        catch { }
                    }
                    else
                    {
                        try
                        {
                            using var client = new NamedPipeClientStream(".", $"DeskdropIPC_{Environment.UserName}", PipeDirection.Out);
                            client.Connect(1000);
                            using var writer = new StreamWriter(client);
                            writer.WriteLine("--open-dashboard");
                            writer.Flush();
                        }
                        catch { }
                    }
                    return;
                }

                Application.SetHighDpiMode(HighDpiMode.PerMonitorV2);
                Application.EnableVisualStyles();
                Application.SetCompatibleTextRenderingDefault(false);
                RegisterProtocolHandler();
                Application.SetUnhandledExceptionMode(UnhandledExceptionMode.CatchException);
                Application.ThreadException += (_, e) => LogError(e.Exception);
                AppDomain.CurrentDomain.UnhandledException += (_, e) =>
                    LogError((Exception)e.ExceptionObject);

                var wpfApp = new System.Windows.Application();
                wpfApp.ShutdownMode = System.Windows.ShutdownMode.OnExplicitShutdown;
            wpfApp.DispatcherUnhandledException += (_, e) => 
            {
                LogError(e.Exception);
                e.Handled = true;
            };

            // Setup Taskbar Jump Lists
            var exePath = System.Diagnostics.Process.GetCurrentProcess().MainModule?.FileName;
            if (!string.IsNullOrEmpty(exePath))
            {
                var jumpList = new System.Windows.Shell.JumpList();
                
                var sendFileTask = new System.Windows.Shell.JumpTask
                {
                    Title = "Send a File",
                    Description = "Send a file to connected devices",
                    ApplicationPath = exePath,
                    Arguments = "--send-file-dialog",
                    IconResourcePath = exePath,
                    CustomCategory = "Quick Actions"
                };

                var syncClipboardTask = new System.Windows.Shell.JumpTask
                {
                    Title = "Sync Clipboard",
                    Description = "Push current clipboard to devices",
                    ApplicationPath = exePath,
                    Arguments = "--sync-clipboard",
                    IconResourcePath = exePath,
                    CustomCategory = "Quick Actions"
                };

                var dashboardTask = new System.Windows.Shell.JumpTask
                {
                    Title = "Open Dashboard",
                    Description = "View transfers and ecosystem",
                    ApplicationPath = exePath,
                    Arguments = "--open-dashboard",
                    IconResourcePath = exePath,
                    CustomCategory = "Quick Actions"
                };

                jumpList.JumpItems.Add(sendFileTask);
                jumpList.JumpItems.Add(syncClipboardTask);
                jumpList.JumpItems.Add(dashboardTask);
                jumpList.ShowFrequentCategory = false;
                jumpList.ShowRecentCategory = false;
                System.Windows.Shell.JumpList.SetJumpList(wpfApp, jumpList);
            }
            
            var trayApp = new TrayApp();
            
            // Start Named Pipe Server for IPC
            Task.Run(() => StartIpcServer(trayApp));

            // Handle arguments for this first instance
            if (args.Length > 0)
            {
                HandleCommandLine(args, trayApp);
            }
            else
            {
                trayApp.OpenDashboard();
            }
            
            wpfApp.Run();
            }
            catch (Exception ex)
            {
                LogError(ex);
            }
        }

        private static void HandleCommandLine(string[] args, TrayApp app)
        {
            if (args.Length >= 2 && args[0] == "--push-file")
            {
                var file = args[1];
                if (File.Exists(file))
                {
                    Task.Run(() => {
                        app.PushFileExternal(file);
                    });
                }
            }
            else if (args.Length >= 1 && args[0].StartsWith("deskdrop://"))
            {
                try
                {
                    var uri = new Uri(args[0]);
                    if (uri.Host == "tofu")
                    {
                        var query = System.Web.HttpUtility.ParseQueryString(uri.Query);
                        var action = query["action"];
                        var deviceId = query["device_id"];
                        if (action == "accept" && !string.IsNullOrEmpty(deviceId))
                        {
                            app.RespondToTrustExternal(deviceId, true);
                        }
                        else if (action == "reject" && !string.IsNullOrEmpty(deviceId))
                        {
                            app.RespondToTrustExternal(deviceId, false);
                        }
                    }
                    else if (uri.Host == "pair")
                    {
                        System.Windows.Application.Current.Dispatcher.Invoke(() => app.OpenDashboard());
                        // Trigger pairing logic if necessary
                    }
                }
                catch (Exception ex)
                {
                    LogError(ex);
                }
            }
            else if (args.Length >= 1 && args[0] == "--send-file-dialog")
            {
                System.Windows.Application.Current.Dispatcher.Invoke(() => app.OpenSendFileDialog());
            }
            else if (args.Length >= 1 && args[0] == "--open-dashboard")
            {
                System.Windows.Application.Current.Dispatcher.Invoke(() => app.OpenDashboard());
            }
            else if (args.Length >= 1 && args[0] == "--sync-clipboard")
            {
                Task.Run(() => app.PushClipboardExternal());
            }
            else if (args.Length >= 1 && args[0] == "--hidden")
            {
                // do nothing, just run in background
            }
        }

        private static void RegisterProtocolHandler()
        {
            try
            {
                var exePath = System.Diagnostics.Process.GetCurrentProcess().MainModule?.FileName;
                if (string.IsNullOrEmpty(exePath)) return;

                using var key = Microsoft.Win32.Registry.CurrentUser.CreateSubKey(@"Software\Classes\deskdrop");
                if (key != null)
                {
                    key.SetValue("", "URL:Deskdrop Protocol");
                    key.SetValue("URL Protocol", "");

                    using var defaultIcon = key.CreateSubKey("DefaultIcon");
                    if (defaultIcon != null) defaultIcon.SetValue("", $"\"{exePath}\",1");

                    using var command = key.CreateSubKey(@"shell\open\command");
                    if (command != null) command.SetValue("", $"\"{exePath}\" \"%1\"");
                }
            }
            catch (Exception ex)
            {
                LogError(ex);
            }
        }

        private static async Task StartIpcServer(TrayApp app)
        {
            while (true)
            {
                try
                {
                    var security = new System.IO.Pipes.PipeSecurity();
                    var user = System.Security.Principal.WindowsIdentity.GetCurrent().User;
                    if (user != null)
                    {
                        security.AddAccessRule(new System.IO.Pipes.PipeAccessRule(user, System.IO.Pipes.PipeAccessRights.FullControl, System.Security.AccessControl.AccessControlType.Allow));
                    }
                    
                    using var server = System.IO.Pipes.NamedPipeServerStreamAcl.Create(
                        $"DeskdropIPC_{Environment.UserName}", 
                        PipeDirection.In, 
                        1, 
                        PipeTransmissionMode.Message, 
                        PipeOptions.Asynchronous, 
                        0, 
                        0, 
                        security);
                        
                    await server.WaitForConnectionAsync();
                    
                    using var reader = new StreamReader(server);
                    var line = await reader.ReadLineAsync();
                    if (!string.IsNullOrEmpty(line))
                    {
                        var parts = line.Split('|');
                        HandleCommandLine(parts, app);
                    }
                }
                catch (Exception ex)
                {
                    LogError(ex);
                    await Task.Delay(1000);
                }
            }
        }

        internal static void LogError(Exception ex)
        {
            try
            {
                var dir = Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                    "Deskdrop");
                Directory.CreateDirectory(dir);
                File.AppendAllText(Path.Combine(dir, "error.log"),
                    $"[{DateTime.Now:u}] {ex.GetType().Name}: {ex.Message}\n{ex.StackTrace}\n\n");
            }
            catch { }
        }
    }
}
