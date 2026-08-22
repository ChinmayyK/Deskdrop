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

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int RegisterWindowMessage(string lpString);

        // Hidden message-only window so the tray icon can survive
        // explorer.exe crashing/restarting: Windows broadcasts
        // "TaskbarCreated" to every top-level window when a new taskbar
        // is created, which is the documented way to know your icon was
        // wiped and needs to be re-added.
        private sealed class TrayMessageWindow : NativeWindow
        {
            private readonly int _taskbarCreatedMsg;
            private readonly Action _onTaskbarCreated;

            public TrayMessageWindow(Action onTaskbarCreated)
            {
                _taskbarCreatedMsg = RegisterWindowMessage("TaskbarCreated");
                _onTaskbarCreated = onTaskbarCreated;
                CreateHandle(new CreateParams());
            }

            protected override void WndProc(ref Message m)
            {
                if (_taskbarCreatedMsg != 0 && m.Msg == _taskbarCreatedMsg)
                {
                    try { _onTaskbarCreated(); } catch { }
                }
                base.WndProc(ref m);
            }
        }

        private static TrayMessageWindow? _messageWindow;
        private static string _logPath = "";

        private static void Log(string msg)
        {
            try
            {
                if (string.IsNullOrEmpty(_logPath))
                {
                    var dir = Path.Combine(
                        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                        "Deskdrop");
                    Directory.CreateDirectory(dir);
                    _logPath = Path.Combine(dir, "tray_debug.txt");
                }
                File.AppendAllText(_logPath, $"[{DateTime.Now:HH:mm:ss.fff}] {msg}\r\n");
            }
            catch { }
        }

        private static Icon LoadTrayIcon()
        {
            var baseDir = AppContext.BaseDirectory;
            string[] iconCandidates = {
                Path.Combine(baseDir, "Assets", "TrayIcon.ico"),
                Path.Combine(baseDir, "TrayIcon.ico"),
                Path.Combine(baseDir, "Assets", "AppIcon.ico"),
                Path.Combine(baseDir, "AppIcon.ico"),
                Path.Combine(baseDir, "Assets", "dark_logo.ico"),
                Path.Combine(baseDir, "dark_logo.ico"),
            };

            foreach (var path in iconCandidates)
            {
                if (File.Exists(path))
                {
                    try
                    {
                        var icon = new Icon(path, SystemInformation.SmallIconSize);
                        Log($"Loaded icon from: {path}");
                        return icon;
                    }
                    catch (Exception ex)
                    {
                        Log($"Failed to load {path}: {ex.Message}");
                    }
                }
            }

            Log("No icon file found, using SystemIcons.Shield");
            return SystemIcons.Shield;
        }

        [STAThread]
        public static void Main()
        {
            Log("=== Deskdrop.Tray starting ===");
            Log($"PID: {Environment.ProcessId}");
            Log($"BaseDirectory: {AppContext.BaseDirectory}");

            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);

            // Catch exceptions from UI-thread event handlers (menu clicks,
            // tray clicks) here instead of letting them crash the process -
            // this helper must never disappear.
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.CatchException);
            Application.ThreadException += (s, e) => Log($"ThreadException (swallowed): {e.Exception}");
            AppDomain.CurrentDomain.UnhandledException += (s, e) =>
                Log($"UnhandledException (terminating={e.IsTerminating}): {e.ExceptionObject}");

            var notifyIcon = new NotifyIcon();
            notifyIcon.Icon = LoadTrayIcon();
            notifyIcon.Text = "Deskdrop";

            _messageWindow = new TrayMessageWindow(() =>
            {
                Log("TaskbarCreated received (explorer restarted) - re-adding tray icon");
                notifyIcon.Visible = false;
                notifyIcon.Icon = LoadTrayIcon();
                notifyIcon.Visible = true;
            });

            // Context menu
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
                notifyIcon.Dispose();
                try
                {
                    foreach (var p in Process.GetProcessesByName("Deskdrop"))
                        p.Kill();
                }
                catch { }
                Application.Exit();
            }));

            notifyIcon.ContextMenuStrip = menu;
            notifyIcon.MouseClick += (s, e) =>
            {
                if (e.Button == MouseButtons.Left)
                    OpenDeskdropWindow();
            };
            notifyIcon.DoubleClick += (s, e) => OpenDeskdropWindow();

            notifyIcon.Visible = true;
            Log($"NotifyIcon.Visible = {notifyIcon.Visible}");
            Log($"NotifyIcon.Icon null = {notifyIcon.Icon == null}");
            if (notifyIcon.Icon != null)
                Log($"NotifyIcon.Icon size = {notifyIcon.Icon.Width}x{notifyIcon.Icon.Height}");

            notifyIcon.ShowBalloonTip(3000, "Deskdrop", "Deskdrop is running in your system tray.", ToolTipIcon.Info);
            Log("Balloon sent. Entering Application.Run()...");

            // IPC listener
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
                        if (!string.IsNullOrEmpty(line) && line == "QUIT")
                        {
                            notifyIcon.Visible = false;
                            notifyIcon.Dispose();
                            Application.Exit();
                            return;
                        }
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
            try
            {
                var baseDir = AppContext.BaseDirectory;
                var exePath = Path.Combine(baseDir, "..", "Deskdrop.exe");
                if (!File.Exists(exePath))
                    exePath = Path.Combine(baseDir, "Deskdrop.exe");

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
                        hWnd = FindWindowW(null, "Deskdrop");
                    if (hWnd == IntPtr.Zero)
                        hWnd = FindWindowW(null, "DeskDrop Dashboard");

                    if (hWnd != IntPtr.Zero)
                    {
                        ShowWindow(hWnd, 9);
                        SetForegroundWindow(hWnd);
                        return;
                    }
                }

                if (File.Exists(exePath))
                {
                    Process.Start(new ProcessStartInfo
                    {
                        FileName = Path.GetFullPath(exePath),
                        UseShellExecute = true
                    });
                }
            }
            catch (Exception ex)
            {
                Log($"OpenDeskdropWindow failed: {ex.Message}");
            }
        }
    }
}
