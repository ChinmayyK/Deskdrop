using System;
using System.Diagnostics;
using System.Drawing;
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
        [DllImport("user32.dll")]
        private static extern bool SetForegroundWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        private static extern IntPtr FindWindowW(string? lpClassName, string? lpWindowName);

        [STAThread]
        public static void Main()
        {
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);

            using var notifyIcon = new NotifyIcon();

            var baseDir = AppContext.BaseDirectory;
            var icoPath = Path.Combine(baseDir, "Assets", "AppIcon.ico");
            if (File.Exists(icoPath))
            {
                try
                {
                    notifyIcon.Icon = new Icon(icoPath, 32, 32);
                }
                catch
                {
                    notifyIcon.Icon = SystemIcons.Application;
                }
            }
            else
            {
                notifyIcon.Icon = SystemIcons.Application;
            }

            notifyIcon.Text = "Deskdrop - Connected & Ready";

            var menu = new ContextMenuStrip();
            var openItem = new ToolStripMenuItem("Open Deskdrop", null, (s, e) => OpenDeskdropWindow());
            openItem.Font = new Font(openItem.Font, FontStyle.Bold);
            menu.Items.Add(openItem);

            menu.Items.Add(new ToolStripMenuItem("Settings...", null, (s, e) => OpenDeskdropWindow()));
            menu.Items.Add(new ToolStripMenuItem("Rescan Network", null, async (s, e) =>
            {
                try
                {
                    using var client = new HttpClient { Timeout = TimeSpan.FromSeconds(2) };
                    await client.PostAsync("http://127.0.0.1:51151/peers/rescan", null);
                }
                catch { }
            }));
            menu.Items.Add(new ToolStripSeparator());
            menu.Items.Add(new ToolStripMenuItem("Quit Deskdrop", null, (s, e) =>
            {
                notifyIcon.Visible = false;
                try
                {
                    foreach (var p in Process.GetProcessesByName("Deskdrop"))
                    {
                        p.Kill();
                    }
                }
                catch { }
                Application.Exit();
            }));

            notifyIcon.ContextMenuStrip = menu;

            notifyIcon.MouseClick += (s, e) =>
            {
                if (e.Button == MouseButtons.Left)
                {
                    OpenDeskdropWindow();
                }
            };
            notifyIcon.DoubleClick += (s, e) => OpenDeskdropWindow();

            notifyIcon.Visible = true;
            notifyIcon.ShowBalloonTip(2000, "Deskdrop", "Deskdrop is running in your System Tray", ToolTipIcon.Info);

            // Named pipe for tooltip & notifications
            var ipcThread = new Thread(() =>
            {
                while (true)
                {
                    try
                    {
                        using var server = new NamedPipeServerStream("Deskdrop_Tray_Pipe", PipeDirection.In);
                        server.WaitForConnection();
                        using var reader = new StreamReader(server);
                        var line = reader.ReadLine();
                        if (!string.IsNullOrEmpty(line) && notifyIcon.Visible)
                        {
                            if (line.StartsWith("TIP:"))
                            {
                                var tip = line.Substring(4);
                                notifyIcon.Text = tip.Length > 63 ? tip.Substring(0, 63) : tip;
                            }
                            else if (line.StartsWith("NOTIFY:"))
                            {
                                var msg = line.Substring(7);
                                notifyIcon.ShowBalloonTip(3000, "Deskdrop", msg, ToolTipIcon.Info);
                            }
                        }
                    }
                    catch
                    {
                        Thread.Sleep(1000);
                    }
                }
            })
            { IsBackground = true };
            ipcThread.Start();

            Application.Run();
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

            if (File.Exists(exePath))
            {
                Process.Start(new ProcessStartInfo
                {
                    FileName = exePath,
                    UseShellExecute = true
                });
            }
        }
    }
}
