using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.IO;
using System.Threading;
using System.Windows.Forms;

namespace Deskdrop.TrayHelper
{
    public class TrayManager : IDisposable
    {
        private Thread? _trayThread;
        private AutoResetEvent _readyEvent = new AutoResetEvent(false);
        private TrayApplicationContext? _context;

        public event Action? OpenRequested;
        public event Action? SettingsRequested;
        public event Action? RescanRequested;
        public event Action? ExitRequested;

        public static TrayManager? Instance { get; private set; }

        public TrayManager()
        {
            Instance = this;
            StartTrayThread();
        }

        private void StartTrayThread()
        {
            _trayThread = new Thread(() =>
            {
                try
                {
                    Application.EnableVisualStyles();
                    Application.SetCompatibleTextRenderingDefault(false);

                    _context = new TrayApplicationContext(this);
                    _readyEvent.Set();

                    Application.Run(_context);
                }
                catch (Exception ex)
                {
                    Log("Tray thread fatal error: " + ex);
                    _readyEvent.Set();
                }
            });

            _trayThread.IsBackground = true;
            _trayThread.SetApartmentState(ApartmentState.STA);
            _trayThread.Start();
            _readyEvent.WaitOne(4000);
        }

        internal void RaiseOpen() => OpenRequested?.Invoke();
        internal void RaiseSettings() => SettingsRequested?.Invoke();
        internal void RaiseRescan() => RescanRequested?.Invoke();
        internal void RaiseExit() => ExitRequested?.Invoke();

        public void UpdateTooltip(string text)
        {
            _context?.UpdateTooltip(text);
        }

        public void ShowNotification(string title, string text)
        {
            _context?.ShowNotification(title, text);
        }

        private static void Log(string msg)
        {
            try
            {
                var dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Deskdrop");
                if (!Directory.Exists(dir)) Directory.CreateDirectory(dir);
                File.AppendAllText(Path.Combine(dir, "winui_trace.txt"), $"[{DateTime.UtcNow:u}] [TrayManager] {msg}\n");
            }
            catch { }
        }

        public void Dispose()
        {
            try
            {
                _context?.ExitThread();
            }
            catch { }
        }

        private class TrayApplicationContext : ApplicationContext
        {
            private readonly TrayManager _parent;
            private readonly NotifyIcon _notifyIcon;
            private readonly IntPtr _hIcon = IntPtr.Zero;

            [System.Runtime.InteropServices.DllImport("user32.dll")]
            private static extern bool DestroyIcon(IntPtr hIcon);

            public TrayApplicationContext(TrayManager parent)
            {
                _parent = parent;
                _notifyIcon = new NotifyIcon();

                var baseDir = AppContext.BaseDirectory;
                var pngPath = Path.Combine(baseDir, "Assets", "Square44x44Logo.scale-200.png");
                if (!File.Exists(pngPath))
                {
                    pngPath = Path.Combine(baseDir, "Assets", "Square150x150Logo.scale-200.png");
                }

                if (File.Exists(pngPath))
                {
                    try
                    {
                        using (var srcBmp = (Bitmap)Image.FromFile(pngPath))
                        {
                            var smallSize = SystemInformation.SmallIconSize;
                            int w = Math.Max(16, smallSize.Width);
                            int h = Math.Max(16, smallSize.Height);

                            using (var targetBmp = new Bitmap(w, h, PixelFormat.Format32bppArgb))
                            {
                                using (var g = Graphics.FromImage(targetBmp))
                                {
                                    g.SmoothingMode = SmoothingMode.HighQuality;
                                    g.InterpolationMode = InterpolationMode.HighQualityBicubic;
                                    g.PixelOffsetMode = PixelOffsetMode.HighQuality;
                                    g.DrawImage(srcBmp, new Rectangle(0, 0, w, h));
                                }

                                _hIcon = targetBmp.GetHicon();
                                _notifyIcon.Icon = (Icon)Icon.FromHandle(_hIcon).Clone();
                            }
                        }
                    }
                    catch (Exception ex)
                    {
                        Log("Icon generation failed: " + ex.Message);
                        _notifyIcon.Icon = SystemIcons.Application;
                    }
                }
                else
                {
                    _notifyIcon.Icon = SystemIcons.Application;
                }

                _notifyIcon.Text = "Deskdrop - Connected & Ready";

                var menu = new ContextMenuStrip();
                var openItem = new ToolStripMenuItem("Open Deskdrop", null, (s, e) => _parent.RaiseOpen());
                openItem.Font = new Font(openItem.Font, FontStyle.Bold);
                menu.Items.Add(openItem);

                menu.Items.Add(new ToolStripMenuItem("Settings...", null, (s, e) => _parent.RaiseSettings()));
                menu.Items.Add(new ToolStripMenuItem("Rescan Network", null, (s, e) => _parent.RaiseRescan()));
                menu.Items.Add(new ToolStripSeparator());
                menu.Items.Add(new ToolStripMenuItem("Quit Deskdrop", null, (s, e) => _parent.RaiseExit()));

                _notifyIcon.ContextMenuStrip = menu;

                _notifyIcon.MouseClick += (s, e) =>
                {
                    if (e.Button == MouseButtons.Left)
                    {
                        _parent.RaiseOpen();
                    }
                };

                _notifyIcon.DoubleClick += (s, e) => _parent.RaiseOpen();

                _notifyIcon.Visible = true;
                Log("NotifyIcon registered via ApplicationContext! Visible=" + _notifyIcon.Visible);
            }

            public void UpdateTooltip(string text)
            {
                if (_notifyIcon != null && _notifyIcon.Visible)
                {
                    try { _notifyIcon.Text = text.Length > 63 ? text.Substring(0, 63) : text; } catch { }
                }
            }

            public void ShowNotification(string title, string text)
            {
                if (_notifyIcon != null && _notifyIcon.Visible)
                {
                    try { _notifyIcon.ShowBalloonTip(3000, title, text, ToolTipIcon.Info); } catch { }
                }
            }

            protected override void Dispose(bool disposing)
            {
                if (disposing)
                {
                    _notifyIcon.Visible = false;
                    _notifyIcon.Dispose();
                    if (_hIcon != IntPtr.Zero)
                    {
                        DestroyIcon(_hIcon);
                    }
                }
                base.Dispose(disposing);
            }
        }
    }
}
