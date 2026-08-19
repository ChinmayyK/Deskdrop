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

                    // Load standard system small icon (16x16 or 32x32)
                    var baseDir = AppContext.BaseDirectory;
                    var icoPath = Path.Combine(baseDir, "Assets", "TrayIcon.ico");
                    var appIcoPath = Path.Combine(baseDir, "Assets", "AppIcon.ico");
                    var smallSize = SystemInformation.SmallIconSize;

                    try
                    {
                        if (File.Exists(icoPath))
                        {
                            _notifyIcon.Icon = new Icon(icoPath, smallSize.Width, smallSize.Height);
                        }
                        else if (File.Exists(appIcoPath))
                        {
                            _notifyIcon.Icon = new Icon(appIcoPath, smallSize.Width, smallSize.Height);
                        }
                        else
                        {
                            _notifyIcon.Icon = SystemIcons.Application;
                        }
                    }
                    catch
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
                Application.ExitThread();
            }
            catch { }
        }
    }
}
