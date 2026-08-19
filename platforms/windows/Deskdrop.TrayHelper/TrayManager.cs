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

                    // Load icon
                    var baseDir = AppContext.BaseDirectory;
                    var icoPath = Path.Combine(baseDir, "Assets", "TrayIcon.ico");
                    if (!File.Exists(icoPath))
                    {
                        icoPath = Path.Combine(baseDir, "Assets", "AppIcon.ico");
                    }

                    if (File.Exists(icoPath))
                    {
                        _notifyIcon.Icon = new Icon(icoPath);
                    }
                    else
                    {
                        _notifyIcon.Icon = SystemIcons.Application;
                    }

                    _notifyIcon.Text = "Deskdrop - Ready";

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

                    _notifyIcon.Click += (s, e) =>
                    {
                        if (e is MouseEventArgs me && me.Button == MouseButtons.Left)
                        {
                            OpenRequested?.Invoke();
                        }
                    };

                    _notifyIcon.DoubleClick += (s, e) => OpenRequested?.Invoke();

                    _notifyIcon.Visible = true;
                    Log("NotifyIcon successfully registered and visible in System Tray!");

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
