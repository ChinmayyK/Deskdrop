using System;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.IO;
using System.IO.Pipes;
using System.Net.Http;
using System.Runtime.InteropServices;
using System.Threading;
using System.Windows.Forms;

namespace Deskdrop.Tray
{
    public static class Program
    {
        private static Mutex? _singleInstanceMutex;
        private static NotifyIcon? _notifyIcon;

        [DllImport("user32.dll")]
        private static extern bool SetForegroundWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        private static extern IntPtr FindWindowW(string? lpClassName, string? lpWindowName);

        [DllImport("user32.dll")]
        private static extern bool DestroyIcon(IntPtr hIcon);

        [STAThread]
        public static void Main(string[] args)
        {
            bool isNewInstance;
            _singleInstanceMutex = new Mutex(true, "Deskdrop_Tray_SingleInstance_Mutex", out isNewInstance);
            if (!isNewInstance)
            {
                // Already running
                OpenDeskdropWindow();
                return;
            }

            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);

            _notifyIcon = new NotifyIcon();

            // Load high-resolution 32-bit ARGB icon
            var baseDir = AppContext.BaseDirectory;
            var pngPath = Path.Combine(baseDir, "Assets", "Square44x44Logo.scale-200.png");
            if (!File.Exists(pngPath))
            {
                pngPath = Path.Combine(baseDir, "Assets", "Square150x150Logo.scale-200.png");
            }

            IntPtr hIcon = IntPtr.Zero;
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

                            hIcon = targetBmp.GetHicon();
                            _notifyIcon.Icon = (Icon)Icon.FromHandle(hIcon).Clone();
                        }
                    }
                }
                catch
                {
                    _notifyIcon.Icon = SystemIcons.Application;
                }
            }
            else
            {
                _notifyIcon.Icon = SystemIcons.Application;
            }

            _notifyIcon.Text = "Deskdrop - Connected & Ready";

            // Context menu
            var menu = new ContextMenuStrip();
            var openItem = new ToolStripMenuItem("Open Deskdrop", null, (s, e) => OpenDeskdropWindow());
            openItem.Font = new Font(openItem.Font, FontStyle.Bold);
            menu.Items.Add(openItem);

            menu.Items.Add(new ToolStripMenuItem("Settings...", null, (s, e) => OpenDeskdropSettings()));
            menu.Items.Add(new ToolStripMenuItem("Rescan Network", null, async (s, e) => await RescanNetworkAsync()));
            menu.Items.Add(new ToolStripSeparator());
            menu.Items.Add(new ToolStripMenuItem("Quit Deskdrop", null, (s, e) => QuitAll()));

            _notifyIcon.ContextMenuStrip = menu;

            _notifyIcon.MouseClick += (s, e) =>
            {
                if (e.Button == MouseButtons.Left)
                {
                    OpenDeskdropWindow();
                }
            };

            _notifyIcon.DoubleClick += (s, e) => OpenDeskdropWindow();

            _notifyIcon.Visible = true;

            // Start IPC listener for tooltip updates from main Deskdrop process
            StartIpcListener();

            Application.Run();

            if (hIcon != IntPtr.Zero)
            {
                DestroyIcon(hIcon);
            }
        }

        private static void OpenDeskdropWindow()
        {
            var baseDir = AppContext.BaseDirectory;
            var exePath = Path.Combine(baseDir, "Deskdrop.exe");

            var procs = Process.GetProcessesByName("Deskdrop");
            if (procs.Length > 0)
            {
                IntPtr hWnd = IntPtr.Zero;
                foreach (var p in procs)
                {
                    if (p.MainWindowHandle != IntPtr.Zero)
                    {
                        hWnd = p.MainWindowHandle;
                        break;
                    }
                }

                if (hWnd == IntPtr.Zero)
                {
                    hWnd = FindWindowW(null, "DeskDrop Dashboard");
                }

                if (hWnd != IntPtr.Zero)
                {
                    ShowWindow(hWnd, 9 /* SW_RESTORE */);
                    SetForegroundWindow(hWnd);
                    return;
                }
            }

            // Not running or hidden -> launch
            if (File.Exists(exePath))
            {
                Process.Start(new ProcessStartInfo
                {
                    FileName = exePath,
                    UseShellExecute = true
                });
            }
        }

        private static void OpenDeskdropSettings()
        {
            OpenDeskdropWindow();
        }

        private static async System.Threading.Tasks.Task RescanNetworkAsync()
        {
            try
            {
                using var client = new HttpClient();
                client.Timeout = TimeSpan.FromSeconds(2);
                await client.PostAsync("http://127.0.0.1:51151/peers/rescan", null);
            }
            catch { }
        }

        private static void QuitAll()
        {
            if (_notifyIcon != null)
            {
                _notifyIcon.Visible = false;
                _notifyIcon.Dispose();
            }

            try
            {
                foreach (var p in Process.GetProcessesByName("Deskdrop"))
                {
                    p.Kill();
                }
            }
            catch { }

            Application.Exit();
            Environment.Exit(0);
        }

        private static void StartIpcListener()
        {
            var t = new Thread(() =>
            {
                while (true)
                {
                    try
                    {
                        using var server = new NamedPipeServerStream("Deskdrop_Tray_Pipe", PipeDirection.In);
                        server.WaitForConnection();
                        using var reader = new StreamReader(server);
                        var line = reader.ReadLine();
                        if (!string.IsNullOrEmpty(line) && _notifyIcon != null && _notifyIcon.Visible)
                        {
                            if (line.StartsWith("TIP:"))
                            {
                                var tip = line.Substring(4);
                                _notifyIcon.Text = tip.Length > 63 ? tip.Substring(0, 63) : tip;
                            }
                            else if (line.StartsWith("NOTIFY:"))
                            {
                                var msg = line.Substring(7);
                                _notifyIcon.ShowBalloonTip(3000, "Deskdrop", msg, ToolTipIcon.Info);
                            }
                        }
                    }
                    catch
                    {
                        Thread.Sleep(1000);
                    }
                }
            });
            t.IsBackground = true;
            t.Start();
        }
    }
}
