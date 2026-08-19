using System;
using System.Drawing;
using System.IO;
using System.Threading;
using System.Windows.Forms;

namespace Deskdrop.TrayHelper
{
    public class TrayManager : IDisposable
    {
        private NotifyIcon? _notifyIcon;
        private Thread? _trayThread;
        private AutoResetEvent _readyEvent = new AutoResetEvent(false);

        public event Action? OpenRequested;
        public event Action? SettingsRequested;
        public event Action? RescanRequested;
        public event Action? ExitRequested;

        public static TrayManager? Instance { get; private set; }

        [System.Runtime.InteropServices.DllImport("user32.dll")]
        private static extern bool DestroyIcon(IntPtr hIcon);

        private IntPtr _createdHicon = IntPtr.Zero;

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
                    _notifyIcon = new NotifyIcon();

                    // Load high-resolution 32-bit ARGB vibrant icon directly from PNG asset
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

                                using (var targetBmp = new Bitmap(w, h, System.Drawing.Imaging.PixelFormat.Format32bppArgb))
                                {
                                    using (var g = Graphics.FromImage(targetBmp))
                                    {
                                        g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.HighQuality;
                                        g.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.HighQualityBicubic;
                                        g.PixelOffsetMode = System.Drawing.Drawing2D.PixelOffsetMode.HighQuality;
                                        g.DrawImage(srcBmp, new Rectangle(0, 0, w, h));
                                    }

                                    _createdHicon = targetBmp.GetHicon();
                                    _notifyIcon.Icon = Icon.FromHandle(_createdHicon);
                                }
                            }
                        }
                        catch (Exception ex)
                        {
                            Log("PNG icon load failed: " + ex.Message);
                            _notifyIcon.Icon = SystemIcons.Application;
                        }
                    }
                    else
                    {
                        _notifyIcon.Icon = SystemIcons.Application;
                    }

                    _notifyIcon.Text = "Deskdrop - Connected & Ready";

                    // Context Menu
                    var menu = new ContextMenuStrip();
                    var openItem = new ToolStripMenuItem("Open Deskdrop", null, (s, e) => OpenRequested?.Invoke());
                    openItem.Font = new Font(openItem.Font, FontStyle.Bold);
                    menu.Items.Add(openItem);

                    menu.Items.Add(new ToolStripMenuItem("Settings...", null, (s, e) => SettingsRequested?.Invoke()));
                    menu.Items.Add(new ToolStripMenuItem("Rescan Network", null, (s, e) => RescanRequested?.Invoke()));
                    menu.Items.Add(new ToolStripSeparator());
                    menu.Items.Add(new ToolStripMenuItem("Quit Deskdrop", null, (s, e) => ExitRequested?.Invoke()));

                    _notifyIcon.ContextMenuStrip = menu;

                    _notifyIcon.MouseClick += (s, e) =>
                    {
                        if (e.Button == MouseButtons.Left)
                        {
                            OpenRequested?.Invoke();
                        }
                    };

                    _notifyIcon.DoubleClick += (s, e) => OpenRequested?.Invoke();

                    _notifyIcon.Visible = true;
                    Log("NotifyIcon successfully registered and visible in System Tray!");

                    try
                    {
                        _notifyIcon.ShowBalloonTip(2000, "Deskdrop", "Deskdrop is active in your system tray", ToolTipIcon.Info);
                    }
                    catch { }

                    _readyEvent.Set();

                    Application.Run();
                }
                catch (Exception ex)
                {
                    Log("Tray thread error: " + ex);
                    _readyEvent.Set();
                }
            });

            _trayThread.IsBackground = true;
            _trayThread.SetApartmentState(ApartmentState.STA);
            _trayThread.Start();
            _readyEvent.WaitOne(3000);
        }

        public void UpdateTooltip(string text)
        {
            if (_notifyIcon != null && _notifyIcon.Visible)
            {
                try
                {
                    _notifyIcon.Text = text.Length > 63 ? text.Substring(0, 63) : text;
                }
                catch { }
            }
        }

        public void ShowNotification(string title, string text, ToolTipIcon icon = ToolTipIcon.Info)
        {
            if (_notifyIcon != null && _notifyIcon.Visible)
            {
                try
                {
                    _notifyIcon.ShowBalloonTip(3000, title, text, icon);
                }
                catch { }
            }
        }

        private static void Log(string msg)
        {
            try
            {
                var dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Deskdrop");
                File.AppendAllText(Path.Combine(dir, "winui_trace.txt"), $"[{DateTime.UtcNow:u}] [TrayManager] {msg}\n");
            }
            catch { }
        }

        public void Dispose()
        {
            try
            {
                if (_notifyIcon != null)
                {
                    _notifyIcon.Visible = false;
                    _notifyIcon.Dispose();
                    _notifyIcon = null;
                }
                if (_createdHicon != IntPtr.Zero)
                {
                    DestroyIcon(_createdHicon);
                    _createdHicon = IntPtr.Zero;
                }
                Application.ExitThread();
            }
            catch { }
        }
    }
}
