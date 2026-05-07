// ClipRelay for Windows
// C# wrapper around the Rust core (P/Invoke).
//
// Build: dotnet publish -c Release -r win-x64 --self-contained false
// The Rust DLL (cliprelay_core.dll) must be in the same directory as the EXE.

using System;
using System.Drawing;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Forms;
using Microsoft.Win32;

namespace ClipRelay.Windows
{
    // ── P/Invoke declarations ────────────────────────────────────────────────

    internal static class NativeCore
    {
        private const string DLL = "cliprelay_core";

        // Event codes
        public const int PB_EVENT_NONE              = 0;
        public const int PB_EVENT_CLIPBOARD_TEXT    = 1;
        public const int PB_EVENT_CLIPBOARD_IMAGE   = 2;
        public const int PB_EVENT_CLIPBOARD_FILE    = 3;
        public const int PB_EVENT_TOFU_PROMPT       = 4;
        public const int PB_EVENT_PEER_CONNECTED    = 5;
        public const int PB_EVENT_PEER_DISCONNECTED = 6;
        public const int PB_EVENT_WARNING           = 7;

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr cliprelay_start(
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? deviceName,
            ushort port);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern void cliprelay_stop(IntPtr handle);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int cliprelay_push_text(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string text);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int cliprelay_push_image(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string mimeType,
            byte[] data,
            UIntPtr len);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int cliprelay_push_file(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string name,
            byte[] data,
            UIntPtr len);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr cliprelay_poll_event(IntPtr handle);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern int cliprelay_event_type(IntPtr eventPtr);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr cliprelay_event_text(IntPtr eventPtr);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr cliprelay_event_device_name(IntPtr eventPtr);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr cliprelay_event_fingerprint(IntPtr eventPtr);

        [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
        public static extern void cliprelay_free_event(IntPtr eventPtr);

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
        private bool _suppressNext;

        public event Action<string>? StatusChanged;
        public event Action<string, string>? TofuPromptRequested; // (deviceName, fingerprint)

        public void Start(string? deviceName = null)
        {
            _handle = NativeCore.cliprelay_start(deviceName, 0);
            if (_handle == IntPtr.Zero)
            {
                StatusChanged?.Invoke("❌ Engine failed to start");
                return;
            }

            StatusChanged?.Invoke("✅ ClipRelay running");

            // Poll Rust event queue at 50 Hz.
            _pollTimer = new System.Threading.Timer(_ => DrainEvents(), null, 0, 20);

            // Watch Windows clipboard via sequence number.
            _lastSequenceNumber = GetClipboardSequenceNumber();
            _watchTimer = new System.Threading.Timer(_ => CheckClipboard(), null, 0, 100);
        }

        public void Stop()
        {
            _pollTimer?.Dispose();
            _watchTimer?.Dispose();
            if (_handle != IntPtr.Zero)
            {
                NativeCore.cliprelay_stop(_handle);
                _handle = IntPtr.Zero;
            }
        }

        public void Dispose() => Stop();

        // ── Outgoing ─────────────────────────────────────────────────────────

        private void CheckClipboard()
        {
            uint seq = GetClipboardSequenceNumber();
            if (seq == _lastSequenceNumber) return;
            _lastSequenceNumber = seq;

            if (_suppressNext) { _suppressNext = false; return; }

            try
            {
                // Must run on STA thread.
                var thread = new Thread(() =>
                {
                    try
                    {
                        if (Clipboard.ContainsText())
                        {
                            string text = Clipboard.GetText();
                            if (!string.IsNullOrEmpty(text))
                            {
                                NativeCore.cliprelay_push_text(_handle, text);
                                return;
                            }
                        }

                        if (Clipboard.ContainsImage())
                        {
                            using var img = Clipboard.GetImage();
                            if (img != null)
                            {
                                using var ms = new MemoryStream();
                                img.Save(ms, System.Drawing.Imaging.ImageFormat.Png);
                                var bytes = ms.ToArray();
                                NativeCore.cliprelay_push_image(
                                    _handle, "image/png", bytes, (UIntPtr)bytes.Length);
                            }
                            return;
                        }

                        if (Clipboard.ContainsFileDropList())
                        {
                            var files = Clipboard.GetFileDropList();
                            if (files?.Count > 0)
                            {
                                var path = files[0]!;
                                var bytes = File.ReadAllBytes(path);
                                var name = Path.GetFileName(path);
                                NativeCore.cliprelay_push_file(
                                    _handle, name, bytes, (UIntPtr)bytes.Length);
                            }
                        }
                    }
                    catch { /* clipboard access can fail transiently */ }
                });
                thread.SetApartmentState(ApartmentState.STA);
                thread.Start();
            }
            catch { }
        }

        // ── Incoming ──────────────────────────────────────────────────────────

        private void DrainEvents()
        {
            if (_handle == IntPtr.Zero) return;
            while (true)
            {
                var ev = NativeCore.cliprelay_poll_event(_handle);
                if (ev == IntPtr.Zero) break;
                try { HandleEvent(ev); }
                finally { NativeCore.cliprelay_free_event(ev); }
            }
        }

        private void HandleEvent(IntPtr ev)
        {
            int kind = NativeCore.cliprelay_event_type(ev);
            switch (kind)
            {
                case NativeCore.PB_EVENT_CLIPBOARD_TEXT:
                    var text = NativeCore.PtrToUtf8String(NativeCore.cliprelay_event_text(ev));
                    var fromName = NativeCore.PtrToUtf8String(NativeCore.cliprelay_event_device_name(ev)) ?? "Unknown";
                    if (text != null) ApplyText(text, fromName);
                    break;

                case NativeCore.PB_EVENT_TOFU_PROMPT:
                    var devName = NativeCore.PtrToUtf8String(NativeCore.cliprelay_event_device_name(ev)) ?? "Unknown";
                    var fp = NativeCore.PtrToUtf8String(NativeCore.cliprelay_event_fingerprint(ev)) ?? "";
                    TofuPromptRequested?.Invoke(devName, fp);
                    break;

                case NativeCore.PB_EVENT_PEER_CONNECTED:
                    var peerName = NativeCore.PtrToUtf8String(NativeCore.cliprelay_event_device_name(ev)) ?? "Unknown";
                    StatusChanged?.Invoke($"📡 Connected: {peerName}");
                    break;

                case NativeCore.PB_EVENT_PEER_DISCONNECTED:
                    StatusChanged?.Invoke("🔌 A device disconnected");
                    break;
            }
        }

        private void ApplyText(string text, string fromDevice)
        {
            _suppressNext = true;
            var thread = new Thread(() =>
            {
                try { Clipboard.SetText(text); }
                catch { _suppressNext = false; }
            });
            thread.SetApartmentState(ApartmentState.STA);
            thread.Start();
            thread.Join();
            StatusChanged?.Invoke($"📋 Clipboard from {fromDevice}");
        }

        [DllImport("user32.dll")]
        private static extern uint GetClipboardSequenceNumber();
    }

    // ── System-tray application ───────────────────────────────────────────────

    internal sealed class TrayApp : ApplicationContext
    {
        private readonly NotifyIcon _tray;
        private readonly ClipboardManager _clipboard = new();
        private readonly ContextMenuStrip _menu = new();
        private ToolStripMenuItem _statusItem = new();

        public TrayApp()
        {
            _statusItem = new ToolStripMenuItem("Starting…") { Enabled = false };
            var devicesItem = new ToolStripMenuItem("Trusted Devices…");
            var quitItem = new ToolStripMenuItem("Quit ClipRelay");
            quitItem.Click += (_, _) => { _clipboard.Stop(); Application.Exit(); };

            _menu.Items.Add(_statusItem);
            _menu.Items.Add(new ToolStripSeparator());
            _menu.Items.Add(devicesItem);
            _menu.Items.Add(new ToolStripSeparator());
            _menu.Items.Add(quitItem);

            _tray = new NotifyIcon
            {
                Icon = SystemIcons.Application,
                Text = "ClipRelay",
                ContextMenuStrip = _menu,
                Visible = true,
            };

            _clipboard.StatusChanged += msg =>
            {
                _statusItem.Text = msg;
                if (msg.StartsWith("📋"))
                    _tray.ShowBalloonTip(2000, "ClipRelay", msg, ToolTipIcon.Info);
            };

            _clipboard.TofuPromptRequested += (deviceName, fingerprint) =>
            {
                var result = MessageBox.Show(
                    $"A new device wants to share your clipboard.\n\n" +
                    $"Device: {deviceName}\nFingerprint:\n{fingerprint}\n\n" +
                    "Only approve devices you own or trust.",
                    "Trust new device?",
                    MessageBoxButtons.YesNo,
                    MessageBoxIcon.Warning);
                // result handled by engine auto-trust; full impl passes decision back.
            };

            _clipboard.Start(Environment.MachineName);
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing) { _tray.Dispose(); _clipboard.Dispose(); }
            base.Dispose(disposing);
        }
    }

    // ── Entry point ───────────────────────────────────────────────────────────

    internal static class Program
    {
        [STAThread]
        static void Main()
        {
            Application.SetHighDpiMode(HighDpiMode.SystemAware);
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            Application.Run(new TrayApp());
        }
    }
}
