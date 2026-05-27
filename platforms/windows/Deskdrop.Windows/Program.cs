// Deskdrop for Windows
// C# wrapper around the Rust core (P/Invoke).
//
// Build: dotnet publish -c Release -r win-x64 --self-contained false
// The Rust DLL (deskdrop_core.dll) must be in the same directory as the EXE.

using System;
using System.Collections.Generic;
using System.Drawing;
using System.IO;
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
        public const int PB_EVENT_NONE                = 0;
        public const int PB_EVENT_CLIPBOARD_TEXT      = 1;
        public const int PB_EVENT_CLIPBOARD_IMAGE     = 2;
        public const int PB_EVENT_CLIPBOARD_FILE      = 3;
        public const int PB_EVENT_TOFU_PROMPT         = 4;
        public const int PB_EVENT_PEER_CONNECTED      = 5;
        public const int PB_EVENT_PEER_DISCONNECTED   = 6;
        public const int PB_EVENT_WARNING             = 7;
        public const int PB_EVENT_CLIPBOARD_AVAILABLE = 11; // timeline-first

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
        public static extern int deskdrop_push_camera_frame(
            IntPtr handle, byte[] data, UIntPtr len);

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

        /// Respond to a TOFU prompt. trust=1 to accept, trust=0 to reject.
        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int deskdrop_trust_peer(
            IntPtr handle, [MarshalAs(UnmanagedType.LPUTF8Str)] string deviceName, int trust);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern void deskdrop_free_event(IntPtr ev);

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
        public event Action<string,string>? TofuPromptRequested;    // (name, fingerprint)
        public event Action<string,string>? ClipboardReceived;      // (text, fromDevice)
        public event Action<HistoryItem>?  HistoryItemAdded;

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

        public void Dispose() => Stop();

        /// Call after the user responds Yes/No to a TOFU dialog.
        public void RespondToTrust(string deviceName, bool trust)
        {
            if (_handle != IntPtr.Zero)
                NativeCore.deskdrop_trust_peer(_handle, deviceName, trust ? 1 : 0);
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
        }

        private void PushLocalClipboard()
        {
            if (_handle == IntPtr.Zero) return;
            try
            {
                if (Clipboard.ContainsText())
                {
                    var text = Clipboard.GetText();
                    if (string.IsNullOrEmpty(text)) return;
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
                    var path  = files[0]!;
                    var bytes = File.ReadAllBytes(path);
                    var name  = Path.GetFileName(path);
                    NativeCore.deskdrop_push_file(_handle, name, bytes, (UIntPtr)bytes.Length);
                    AddHistory(new HistoryItem
                    {
                        Summary = name, Source = "local",
                        Time = DateTime.Now, TypeIcon = "📎",
                    });
                }
            }
            catch { /* clipboard is inherently racy on Windows */ }
        }

        public void PushFile(string path)
        {
            if (_handle == IntPtr.Zero || !File.Exists(path)) return;
            var bytes = File.ReadAllBytes(path);
            var name  = Path.GetFileName(path);
            NativeCore.deskdrop_push_file(_handle, name, bytes, (UIntPtr)bytes.Length);
            AddHistory(new HistoryItem
            {
                Summary = name, Source = "local",
                Time = DateTime.Now, TypeIcon = "📎",
            });
        }

        public void PushCameraFrame(byte[] jpegBytes)
        {
            if (_handle == IntPtr.Zero) return;
            NativeCore.deskdrop_push_camera_frame(_handle, jpegBytes, (UIntPtr)jpegBytes.Length);
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

                // New device wants to pair.
                case NativeCore.PB_EVENT_TOFU_PROMPT:
                {
                    var name = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_name(ev)) ?? "Unknown";
                    var fp   = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_fingerprint(ev)) ?? "";
                    TofuPromptRequested?.Invoke(name, fp);
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
            }
        }

        private void ApplyText(string text, string fromDevice)
        {
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
        }

        private void AddHistory(HistoryItem item)
        {
            lock (_histLock)
            {
                _history.RemoveAll(i => i.FullText != null && i.FullText == item.FullText);
                _history.Insert(0, item);
                if (_history.Count > 100) _history.RemoveRange(100, _history.Count - 100);
            }
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
        private readonly ToolStripMenuItem _syncToggleItem;

        private MainWindow?            _mainWindow;
        private bool                   _syncEnabled  = true;
        private DateTime               _lastBalloonAt = DateTime.MinValue;

        public TrayApp()
        {
            _statusItem = new ToolStripMenuItem("Starting…") { Enabled = false };

            _sendItem = new ToolStripMenuItem("Send Clipboard to Devices") { Enabled = false };
            _sendItem.Click += OnSendClipboard;

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

            _menu.Items.AddRange(new ToolStripItem[]
            {
                _statusItem,
                new ToolStripSeparator(),
                _sendItem,
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

            _mgr.StatusChanged       += OnStatusChanged;
            _mgr.TofuPromptRequested += OnTofuPrompt;
            _mgr.HistoryItemAdded    += item => {
                if (!LoadSettings().ShowNotifications) return;
                System.Windows.Application.Current.Dispatcher.Invoke(() =>
                {
                    string title = item.Source == "local" ? "Sent Clipboard" : $"Received from {item.Source}";
                    _tray.ShowBalloonTip(2000, title, $"{item.TypeIcon} {item.Summary}", ToolTipIcon.Info);
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
                _tray.ShowBalloonTip(3000, $"📋 Clipboard from {from}", preview, ToolTipIcon.Info));
        }

        // ── TOFU ─────────────────────────────────────────────────────────────

        private void OnTofuPrompt(string deviceName, string fingerprint)
        {
            System.Windows.Application.Current.Dispatcher.Invoke(() =>
            {
                OpenDashboard();
                if (_mainWindow != null)
                {
                    _mainWindow.ShowTofuPrompt(deviceName, fingerprint);
                }
            });
        }

        // ── Menu actions ──────────────────────────────────────────────────────

        private void OnSendClipboard(object? s, EventArgs e)
        {
            // Use named pipe to tell the running daemon to push its current clipboard.
            Task.Run(() =>
            {
                bool ok = DaemonClient.Send(new { cmd = "push_clipboard" }) != null;
                if (ok)
                    System.Windows.Application.Current.Dispatcher.Invoke(() =>
                        _tray.ShowBalloonTip(1500, "Deskdrop", "Clipboard sent.", ToolTipIcon.Info));
            });
        }

        private void OnToggleSync(object? s, EventArgs e)
        {
            _syncEnabled = !_syncEnabled;
            _syncToggleItem.Text = _syncEnabled ? "Pause Sync" : "Resume Sync";
            using var k = Registry.CurrentUser.CreateSubKey(@"Software\Deskdrop");
            k.SetValue("SyncEnabled", _syncEnabled ? 1 : 0, RegistryValueKind.DWord);
            Task.Run(() => DaemonClient.Send(new { cmd = "save_settings", sync_enabled = _syncEnabled }));
            _tray.ShowBalloonTip(1500, "Deskdrop",
                _syncEnabled ? "Clipboard sync resumed." : "Clipboard sync paused.", ToolTipIcon.Info);
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
            _tray.ShowBalloonTip(1500, "Deskdrop", "Scanning for nearby devices…", ToolTipIcon.Info);
        }

        // ── Dashboard panel ─────────────────────────────────────────────────────

        private void OpenDashboard()
        {
            if (_mainWindow != null && _mainWindow.IsLoaded)
            {
                if (_mainWindow.Visibility != System.Windows.Visibility.Visible)
                    _mainWindow.Show();
                if (_mainWindow.WindowState == System.Windows.WindowState.Minimized)
                    _mainWindow.WindowState = System.Windows.WindowState.Normal;
                _mainWindow.Activate();
                return;
            }

            _mainWindow = new MainWindow(_mgr);
            _mainWindow.Show();
        }

        // ── Settings ──────────────────────────────────────────────────────────

        public record AppSettings(bool SyncEnabled, bool ShowNotifications,
            string DeviceName, ushort Port, bool HasCompletedOnboarding);

        public static AppSettings LoadSettings()
        {
            using var key = Registry.CurrentUser.OpenSubKey(@"Software\Deskdrop");
            if (key == null) return new AppSettings(true, true, "", 47823, false);
            return new AppSettings(
                SyncEnabled:       ((int?)key.GetValue("SyncEnabled",       1) ?? 1) != 0,
                ShowNotifications: ((int?)key.GetValue("ShowNotifications", 1) ?? 1) != 0,
                DeviceName:        (string?)key.GetValue("DeviceName", "") ?? "",
                Port:              (ushort)Math.Clamp(
                    (int?)key.GetValue("Port", 47823) ?? 47823, 1024, 65535),
                HasCompletedOnboarding: ((int?)key.GetValue("HasCompletedOnboarding", 0) ?? 0) != 0);
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

        protected override void Dispose(bool disposing)
        {
            if (disposing) { _tray.Dispose(); _mgr.Dispose(); _menu.Dispose(); }
            base.Dispose(disposing);
        }
    }

    // ── Native helpers ────────────────────────────────────────────────────────

    internal static class NativeMethods
    {
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool DestroyIcon(IntPtr hIcon);
    }

    // ── Entry point ───────────────────────────────────────────────────────────

    internal static class Program
    {
        [STAThread]
        static void Main()
        {
            // Single-instance guard.
            using var mutex = new Mutex(true, "Deskdrop_SingleInstance_v1", out bool isNew);
            if (!isNew)
            {
                System.Windows.Forms.MessageBox.Show("Deskdrop is already running in the system tray.",
                    "Already running", MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }

            Application.SetHighDpiMode(HighDpiMode.PerMonitorV2);
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.CatchException);
            Application.ThreadException += (_, e) => LogError(e.Exception);
            AppDomain.CurrentDomain.UnhandledException += (_, e) =>
                LogError((Exception)e.ExceptionObject);

            var wpfApp = new System.Windows.Application();
            wpfApp.ShutdownMode = System.Windows.ShutdownMode.OnExplicitShutdown;
            
            var trayApp = new TrayApp();
            
            // Run WinForms loop so NotifyIcon works naturally, 
            // WPF elements will piggyback via WindowsFormsSynchronizationContext
            Application.Run(trayApp);
        }

        private static void LogError(Exception ex)
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
