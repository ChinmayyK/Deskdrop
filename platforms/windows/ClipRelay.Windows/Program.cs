// ClipRelay for Windows
// C# wrapper around the Rust core (P/Invoke).
//
// Build: dotnet publish -c Release -r win-x64 --self-contained false
// The Rust DLL (cliprelay_core.dll) must be in the same directory as the EXE.

using System;
using System.Collections.Generic; (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
using System.Drawing;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Threading.Tasks; (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
using System.Windows.Forms;
using Microsoft.Win32;

namespace ClipRelay.Windows
{
    // ── P/Invoke declarations ────────────────────────────────────────────────

    internal static class NativeCore
    {
        private const string DLL = "cliprelay_core";

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
        public static extern IntPtr cliprelay_start(
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? deviceName, ushort port); (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern void cliprelay_stop(IntPtr handle);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int cliprelay_push_text(
            IntPtr handle, [MarshalAs(UnmanagedType.LPUTF8Str)] string text);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int cliprelay_push_image(
            IntPtr handle, [MarshalAs(UnmanagedType.LPUTF8Str)] string mimeType,
            byte[] data, UIntPtr len);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int cliprelay_push_file(
            IntPtr handle, [MarshalAs(UnmanagedType.LPUTF8Str)] string name,
            byte[] data, UIntPtr len); (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr cliprelay_poll_event(IntPtr handle);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int cliprelay_event_type(IntPtr ev);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr cliprelay_event_text(IntPtr ev);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr cliprelay_event_device_name(IntPtr ev);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr cliprelay_event_fingerprint(IntPtr ev);

        /// Respond to a TOFU prompt. trust=1 to accept, trust=0 to reject.
        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int cliprelay_trust_peer(
            IntPtr handle, [MarshalAs(UnmanagedType.LPUTF8Str)] string deviceName, int trust);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern void cliprelay_free_event(IntPtr ev); (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)

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

    // ── Clipboard Manager ────────────────────────────────────────────────────

    internal sealed class ClipboardManager : IDisposable
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
            _handle = NativeCore.cliprelay_start(deviceName, port);
            if (_handle == IntPtr.Zero)
            {
                StatusChanged?.Invoke(
                    "❌ Engine failed to start — cliprelay_core.dll missing or incompatible");
                return;
            }

            RefreshStatus();
            _pollTimer  = new System.Threading.Timer(_ => DrainEvents(),    null, 0,   20);
            _watchTimer = new System.Threading.Timer(_ => CheckClipboard(), null, 200, 100);
            _lastSequenceNumber = GetClipboardSequenceNumber(); (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
        }

        public void Stop()
        {
            _pollTimer?.Dispose();  _pollTimer  = null;
            _watchTimer?.Dispose(); _watchTimer = null;
            if (_handle != IntPtr.Zero) { NativeCore.cliprelay_stop(_handle); _handle = IntPtr.Zero; } (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
        }

        public void Dispose() => Stop();

        /// Call after the user responds Yes/No to a TOFU dialog.
        public void RespondToTrust(string deviceName, bool trust)
        {
            if (_handle != IntPtr.Zero)
                NativeCore.cliprelay_trust_peer(_handle, deviceName, trust ? 1 : 0);
            RefreshStatus();
        }

        public List<HistoryItem> GetHistory()
        {
            lock (_histLock) return new List<HistoryItem>(_history);
        }

        // ── Outgoing: watch local clipboard ──────────────────────────────────

        private void CheckClipboard()
        {
            if (_handle == IntPtr.Zero) return; (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
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
                    NativeCore.cliprelay_push_text(_handle, text);
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
                    NativeCore.cliprelay_push_image(_handle, "image/png", bytes, (UIntPtr)bytes.Length);
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
                    NativeCore.cliprelay_push_file(_handle, name, bytes, (UIntPtr)bytes.Length);
                    AddHistory(new HistoryItem
                    {
                        Summary = name, Source = "local",
                        Time = DateTime.Now, TypeIcon = "📎",
                    });
                }
            }
            catch { /* clipboard is inherently racy on Windows */ }
        }

        // ── Incoming: drain Rust event queue ───────────────────────────────── (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)

        private void DrainEvents()
        {
            if (_handle == IntPtr.Zero) return;
            while (true)
            {
                var ev = NativeCore.cliprelay_poll_event(_handle);
                if (ev == IntPtr.Zero) break;
                try   { HandleEvent(ev); } (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
                finally { NativeCore.cliprelay_free_event(ev); }
            }
        }

        private void HandleEvent(IntPtr ev)
        {
            int kind = NativeCore.cliprelay_event_type(ev);
            switch (kind)
            {
                // Text auto-applied (engine decided to apply it immediately).
                case NativeCore.PB_EVENT_CLIPBOARD_TEXT:
                {
                    var text = NativeCore.PtrToUtf8String(NativeCore.cliprelay_event_text(ev));
                    var from = NativeCore.PtrToUtf8String(NativeCore.cliprelay_event_device_name(ev)) ?? "Unknown";
                    if (text != null) ApplyText(text, from);
                    break;
                }

                // Text available (timeline-first): notify user, don't auto-apply.
                case NativeCore.PB_EVENT_CLIPBOARD_AVAILABLE:
                {
                    var text = NativeCore.PtrToUtf8String(NativeCore.cliprelay_event_text(ev));
                    var from = NativeCore.PtrToUtf8String(NativeCore.cliprelay_event_device_name(ev)) ?? "Unknown";
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
                    var name = NativeCore.PtrToUtf8String(NativeCore.cliprelay_event_device_name(ev)) ?? "Unknown";
                    var fp   = NativeCore.PtrToUtf8String(NativeCore.cliprelay_event_fingerprint(ev)) ?? "";
                    TofuPromptRequested?.Invoke(name, fp);
                    break;
                }

                case NativeCore.PB_EVENT_PEER_CONNECTED:
                {
                    var peer = NativeCore.PtrToUtf8String(NativeCore.cliprelay_event_device_name(ev)) ?? "Unknown";
                    lock (_connectedPeers) _connectedPeers.Add(peer);
                    RefreshStatus();
                    break;
                }

                case NativeCore.PB_EVENT_PEER_DISCONNECTED:
                {
                    var peer = NativeCore.PtrToUtf8String(NativeCore.cliprelay_event_device_name(ev));
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
                    var msg = NativeCore.PtrToUtf8String(NativeCore.cliprelay_event_text(ev));
                    if (msg != null) StatusChanged?.Invoke($"⚠️ {msg}");
                    break;
                } (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
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
 (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
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

        private ClipboardHistoryPanel? _historyPanel;
        private bool                   _syncEnabled  = true;
        private DateTime               _lastBalloonAt = DateTime.MinValue; (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)

        public TrayApp()
        {
            _statusItem = new ToolStripMenuItem("Starting…") { Enabled = false };

            _sendItem = new ToolStripMenuItem("Send Clipboard to Devices") { Enabled = false };
            _sendItem.Click += OnSendClipboard;

            _syncToggleItem = new ToolStripMenuItem("Pause Sync");
            _syncToggleItem.Click += OnToggleSync;

            var historyItem = new ToolStripMenuItem("Clipboard History…");
            historyItem.Click += (_, _) => OpenHistoryPanel();

            var scanItem = new ToolStripMenuItem("Scan for Devices");
            scanItem.Click += OnScanDevices;

            var connectItem = new ToolStripMenuItem("Connect to Device…");
            connectItem.Click += OnManualConnect;

            var prefsItem = new ToolStripMenuItem("Preferences…");
            prefsItem.Click += (_, _) => { using var f = new PreferencesForm(); f.ShowDialog(); };

            var quitItem = new ToolStripMenuItem("Quit ClipRelay");
            quitItem.Click += (_, _) => { _mgr.Stop(); Application.Exit(); };

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
                Text             = "ClipRelay",
                ContextMenuStrip = _menu,
                Visible          = true,
            };
            _tray.DoubleClick += (_, _) => OpenHistoryPanel();

            _mgr.StatusChanged       += OnStatusChanged;
            _mgr.TofuPromptRequested += OnTofuPrompt;
            _mgr.ClipboardReceived   += OnClipboardReceived;
            _mgr.HistoryItemAdded    += item =>
                _historyPanel?.BeginInvoke(() => _historyPanel?.AddItem(item));

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
            if (_tray.IsDisposed) return;
            _tray.BeginInvoke(() =>
            {
                _statusItem.Text = msg.Length > 63 ? msg[..60] + "…" : msg;
                bool connected = _mgr.IsConnected();
                _tray.Icon = BuildTrayIcon(connected);
                _tray.Text = connected ? "ClipRelay — syncing" : "ClipRelay — idle";
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
            _tray.BeginInvoke(() =>
                _tray.ShowBalloonTip(3000, $"📋 Clipboard from {from}", preview, ToolTipIcon.Info));
        }

        // ── TOFU ─────────────────────────────────────────────────────────────

        private void OnTofuPrompt(string deviceName, string fingerprint)
        {
            _tray.BeginInvoke(() =>
            {
                string fp = FormatFingerprint(fingerprint);
                var form = new Form
                {
                    Text = "Trust new device?", ClientSize = new Size(430, 330),
                    FormBorderStyle = FormBorderStyle.FixedDialog,
                    StartPosition = FormStartPosition.CenterScreen,
                    MaximizeBox = false, MinimizeBox = false, TopMost = true,
                };
                var lbl = new Label
                {
                    Text = $"A new device wants to sync your clipboard.\n\n" +
                           $"Device: {deviceName}\n\nFingerprint:",
                    Left = 16, Top = 16, Width = 398, Height = 70,
                };
                var fpBox = new TextBox
                {
                    Text = fp, Left = 16, Top = 90, Width = 398, Height = 110,
                    Multiline = true, ReadOnly = true, Font = new Font("Consolas", 9f),
                    BackColor = SystemColors.Window, BorderStyle = BorderStyle.FixedSingle,
                };
                var warn = new Label
                {
                    Text = "⚠️  Only trust devices you own or control.",
                    Left = 16, Top = 208, Width = 398, Height = 24, ForeColor = Color.DarkOrange,
                };
                var btnOk  = new Button { Text = "✅ Trust",  Left = 224, Top = 254, Width = 90, Height = 32, DialogResult = DialogResult.Yes };
                var btnNo  = new Button { Text = "❌ Reject", Left = 322, Top = 254, Width = 90, Height = 32, DialogResult = DialogResult.No  };
                form.Controls.AddRange(new Control[] { lbl, fpBox, warn, btnOk, btnNo });
                form.AcceptButton = btnOk; form.CancelButton = btnNo;

                bool approved = form.ShowDialog() == DialogResult.Yes;
                _mgr.RespondToTrust(deviceName, approved);
                if (approved)
                    _tray.ShowBalloonTip(2000, "ClipRelay", $"{deviceName} trusted.", ToolTipIcon.Info);
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
                    _tray.BeginInvoke(() =>
                        _tray.ShowBalloonTip(1500, "ClipRelay", "Clipboard sent.", ToolTipIcon.Info));
            });
        }

        private void OnToggleSync(object? s, EventArgs e)
        {
            _syncEnabled = !_syncEnabled;
            _syncToggleItem.Text = _syncEnabled ? "Pause Sync" : "Resume Sync";
            using var k = Registry.CurrentUser.CreateSubKey(@"Software\ClipRelay");
            k.SetValue("SyncEnabled", _syncEnabled ? 1 : 0, RegistryValueKind.DWord);
            Task.Run(() => DaemonClient.Send(new { cmd = "save_settings", sync_enabled = _syncEnabled }));
            _tray.ShowBalloonTip(1500, "ClipRelay",
                _syncEnabled ? "Clipboard sync resumed." : "Clipboard sync paused.", ToolTipIcon.Info);
        }

        private void OnManualConnect(object? s, EventArgs e)
        {
            string? addr = ShowInputDialog(
                "Connect to Device",
                "IP address or hostname (optionally :port):",
                "192.168.1.x:47823");
            if (string.IsNullOrWhiteSpace(addr)) return;

            Task.Run(() =>
            {
                var parts = addr.Split(':', 2, StringSplitOptions.RemoveEmptyEntries);
                object cmd = parts.Length == 2 && ushort.TryParse(parts[1], out var p)
                    ? new { cmd = "connect_manual", host = parts[0], port = (int)p }
                    : (object)new { cmd = "connect_manual", host = addr };
                var resp = DaemonClient.Send(cmd);
                _tray.BeginInvoke(() =>
                {
                    if (resp != null)
                        _tray.ShowBalloonTip(2000, "ClipRelay", $"Connecting to {addr}…", ToolTipIcon.Info);
                    else
                        MessageBox.Show($"Could not reach daemon.\nMake sure ClipRelay is running.",
                            "Connection failed", MessageBoxButtons.OK, MessageBoxIcon.Warning);
                });
            });
        }

        private void OnScanDevices(object? s, EventArgs e)
        {
            Task.Run(() => DaemonClient.Send(new { cmd = "rescan_peers" }));
            _tray.ShowBalloonTip(1500, "ClipRelay", "Scanning for nearby devices…", ToolTipIcon.Info);
        }

        // ── History panel ─────────────────────────────────────────────────────

        private void OpenHistoryPanel()
        {
            if (_historyPanel != null && !_historyPanel.IsDisposed)
            {
                _historyPanel.BringToFront(); _historyPanel.Focus(); return;
            }

            _historyPanel = new ClipboardHistoryPanel();

            // Pre-populate with everything collected since startup.
            foreach (var item in _mgr.GetHistory())
                _historyPanel.AddItem(item);

            _historyPanel.RepushRequested += item =>
            {
                if (item.FullText != null)
                {
                    // Write to local clipboard then push via daemon pipe.
                    var t = new Thread(() => { try { Clipboard.SetText(item.FullText); } catch { } });
                    t.SetApartmentState(ApartmentState.STA); t.IsBackground = true; t.Start();
                    Task.Run(() => DaemonClient.Send(new { cmd = "push_clipboard" }));
                }
            };

            var wa = Screen.PrimaryScreen?.WorkingArea ?? new Rectangle(0, 0, 1920, 1080);
            _historyPanel.Show();
            _historyPanel.Location = new Point(
                wa.Right  - _historyPanel.Width  - 12,
                wa.Bottom - _historyPanel.Height - 12);
        }

        // ── Settings ──────────────────────────────────────────────────────────

        private record AppSettings(bool SyncEnabled, bool ShowNotifications,
            string DeviceName, ushort Port);

        private static AppSettings LoadSettings()
        {
            using var key = Registry.CurrentUser.OpenSubKey(@"Software\ClipRelay");
            if (key == null) return new AppSettings(true, true, "", 47823);
            return new AppSettings(
                SyncEnabled:       ((int?)key.GetValue("SyncEnabled",       1) ?? 1) != 0,
                ShowNotifications: ((int?)key.GetValue("ShowNotifications", 1) ?? 1) != 0,
                DeviceName:        (string?)key.GetValue("DeviceName", "") ?? "",
                Port:              (ushort)Math.Clamp(
                    (int?)key.GetValue("Port", 47823) ?? 47823, 1024, 65535));
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

        private static string? ShowInputDialog(string title, string prompt, string placeholder)
        {
            var form = new Form
            {
                Text = title, ClientSize = new Size(380, 120),
                FormBorderStyle = FormBorderStyle.FixedDialog,
                StartPosition = FormStartPosition.CenterScreen,
                MaximizeBox = false, MinimizeBox = false,
            };
            var lbl  = new Label  { Text = prompt, Left = 12, Top = 14, Width = 356, Height = 34 };
            var txt  = new TextBox { Left = 12, Top = 52, Width = 260, PlaceholderText = placeholder };
            var btn  = new Button  { Text = "Connect", Left = 280, Top = 50, Width = 88, Height = 26,
                                     DialogResult = DialogResult.OK };
            form.Controls.AddRange(new Control[] { lbl, txt, btn });
            form.AcceptButton = btn;
            return form.ShowDialog() == DialogResult.OK && !string.IsNullOrWhiteSpace(txt.Text)
                ? txt.Text.Trim() : null; (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing) { _tray.Dispose(); _mgr.Dispose(); _menu.Dispose(); } (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
            base.Dispose(disposing);
        }
    }

    // ── Native helpers ────────────────────────────────────────────────────────

    internal static class NativeMethods
    {
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool DestroyIcon(IntPtr hIcon);
    }
 (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
    // ── Entry point ───────────────────────────────────────────────────────────

    internal static class Program
    {
        [STAThread]
        static void Main()
        {
            // Single-instance guard.
            using var mutex = new Mutex(true, "ClipRelay_SingleInstance_v1", out bool isNew);
            if (!isNew)
            {
                MessageBox.Show("ClipRelay is already running in the system tray.",
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

            Application.Run(new TrayApp());
        }

        private static void LogError(Exception ex)
        {
            try
            {
                var dir = Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                    "ClipRelay");
                Directory.CreateDirectory(dir);
                File.AppendAllText(Path.Combine(dir, "error.log"),
                    $"[{DateTime.Now:u}] {ex.GetType().Name}: {ex.Message}\n{ex.StackTrace}\n\n");
            }
            catch { }
        } (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
    }
}
