// Deskdrop for Windows
// C# wrapper around the Rust core (P/Invoke).
//
// Build: dotnet publish -c Release -r win-x64 --self-contained false
// The Rust DLL (deskdrop_core.dll) must be in the same directory as the EXE.

using System;
using System.Collections.Generic;
using System.Drawing;
using System.IO;
using System.IO.Pipes;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Forms;
using Microsoft.Win32;

namespace Deskdrop.Windows
{
    // ── P/Invoke declarations ────────────────────────────────────────────────

    // ── Entry point ───────────────────────────────────────────────────────────

    internal static class Program
    {
        [STAThread]
        static void Main(string[] args)
        {
            try
            {
                using var mutex = new Mutex(true, $"Deskdrop_SingleInstance_v1_{Environment.UserName}", out bool isNew);
                
                if (!isNew)
                {
                    if (args.Length > 0)
                    {
                        try
                        {
                            using var client = new NamedPipeClientStream(".", $"DeskdropIPC_{Environment.UserName}", PipeDirection.Out);
                            client.Connect(1000);
                            using var writer = new StreamWriter(client);
                            writer.WriteLine(string.Join("|", args));
                            writer.Flush();
                        }
                        catch { }
                    }
                    else
                    {
                        try
                        {
                            using var client = new NamedPipeClientStream(".", $"DeskdropIPC_{Environment.UserName}", PipeDirection.Out);
                            client.Connect(1000);
                            using var writer = new StreamWriter(client);
                            writer.WriteLine("--open-dashboard");
                            writer.Flush();
                        }
                        catch { }
                    }
                    return;
                }

                Application.SetHighDpiMode(HighDpiMode.PerMonitorV2);
                Application.EnableVisualStyles();
                Application.SetCompatibleTextRenderingDefault(false);
                RegisterProtocolHandler();
                FirewallHelper.EnsureRules();
                Application.SetUnhandledExceptionMode(UnhandledExceptionMode.CatchException);
                Application.ThreadException += (_, e) => LogError(e.Exception);
                AppDomain.CurrentDomain.UnhandledException += (_, e) =>
                    LogError((Exception)e.ExceptionObject);

                var wpfApp = new System.Windows.Application();
                wpfApp.ShutdownMode = System.Windows.ShutdownMode.OnExplicitShutdown;
            wpfApp.DispatcherUnhandledException += (_, e) => 
            {
                LogError(e.Exception);
                e.Handled = true;
            };

            // Setup Taskbar Jump Lists
            var exePath = System.Diagnostics.Process.GetCurrentProcess().MainModule?.FileName;
            if (!string.IsNullOrEmpty(exePath))
            {
                var jumpList = new System.Windows.Shell.JumpList();
                
                var sendFileTask = new System.Windows.Shell.JumpTask
                {
                    Title = "Send a File",
                    Description = "Send a file to connected devices",
                    ApplicationPath = exePath,
                    Arguments = "--send-file-dialog",
                    IconResourcePath = exePath,
                    CustomCategory = "Quick Actions"
                };

                var syncClipboardTask = new System.Windows.Shell.JumpTask
                {
                    Title = "Sync Clipboard",
                    Description = "Push current clipboard to devices",
                    ApplicationPath = exePath,
                    Arguments = "--sync-clipboard",
                    IconResourcePath = exePath,
                    CustomCategory = "Quick Actions"
                };

                var dashboardTask = new System.Windows.Shell.JumpTask
                {
                    Title = "Open Dashboard",
                    Description = "View transfers and ecosystem",
                    ApplicationPath = exePath,
                    Arguments = "--open-dashboard",
                    IconResourcePath = exePath,
                    CustomCategory = "Quick Actions"
                };

                jumpList.JumpItems.Add(sendFileTask);
                jumpList.JumpItems.Add(syncClipboardTask);
                jumpList.JumpItems.Add(dashboardTask);
                jumpList.ShowFrequentCategory = false;
                jumpList.ShowRecentCategory = false;
                System.Windows.Shell.JumpList.SetJumpList(wpfApp, jumpList);
            }
            
            var trayApp = new TrayApp();
            
            // Start Named Pipe Server for IPC
            Task.Run(() => StartIpcServer(trayApp));

            // Handle arguments for this first instance
            if (args.Length > 0)
            {
                HandleCommandLine(args, trayApp);
            }
            else
            {
                trayApp.OpenDashboard();
            }
            
            wpfApp.Run();
            }
            catch (Exception ex)
            {
                LogError(ex);
            }
        }

        private static void HandleCommandLine(string[] args, TrayApp app)
        {
            if (args.Length >= 2 && args[0] == "--push-file")
            {
                var file = args[1];
                if (File.Exists(file))
                {
                    Task.Run(() => {
                        app.PushFileExternal(file);
                    });
                }
            }
            else if (args.Length >= 1 && args[0].StartsWith("deskdrop://"))
            {
                try
                {
                    var uri = new Uri(args[0]);
                    if (uri.Host == "tofu" || uri.Host == "pair")
                    {
                        // SECURITY FIX: Never accept trust automatically via command line or URL handler.
                        // Open dashboard so user interactively reviews pending pairing requests.
                        System.Windows.Application.Current.Dispatcher.Invoke(() => app.OpenDashboard());
                    }
                }
                catch (Exception ex)
                {
                    LogError(ex);
                }
            }
            else if (args.Length >= 1 && args[0] == "--send-file-dialog")
            {
                System.Windows.Application.Current.Dispatcher.Invoke(() => app.OpenSendFileDialog());
            }
            else if (args.Length >= 1 && args[0] == "--open-dashboard")
            {
                System.Windows.Application.Current.Dispatcher.Invoke(() => app.OpenDashboard());
            }
            else if (args.Length >= 1 && args[0] == "--sync-clipboard")
            {
                Task.Run(() => app.PushClipboardExternal());
            }
            else if (args.Length >= 1 && args[0] == "--hidden")
            {
                // do nothing, just run in background
            }
        }

        private static void RegisterProtocolHandler()
        {
            try
            {
                var exePath = System.Diagnostics.Process.GetCurrentProcess().MainModule?.FileName;
                if (string.IsNullOrEmpty(exePath)) return;

                using var key = Microsoft.Win32.Registry.CurrentUser.CreateSubKey(@"Software\Classes\deskdrop");
                if (key != null)
                {
                    key.SetValue("", "URL:Deskdrop Protocol");
                    key.SetValue("URL Protocol", "");

                    using var defaultIcon = key.CreateSubKey("DefaultIcon");
                    if (defaultIcon != null) defaultIcon.SetValue("", $"\"{exePath}\",1");

                    using var command = key.CreateSubKey(@"shell\open\command");
                    if (command != null) command.SetValue("", $"\"{exePath}\" \"%1\"");
                }
            }
            catch (Exception ex)
            {
                LogError(ex);
            }
        }

        private static async Task StartIpcServer(TrayApp app)
        {
            while (true)
            {
                try
                {
                    var security = new System.IO.Pipes.PipeSecurity();
                    var user = System.Security.Principal.WindowsIdentity.GetCurrent().User;
                    if (user != null)
                    {
                        security.AddAccessRule(new System.IO.Pipes.PipeAccessRule(user, System.IO.Pipes.PipeAccessRights.FullControl, System.Security.AccessControl.AccessControlType.Allow));
                    }
                    
                    using var server = System.IO.Pipes.NamedPipeServerStreamAcl.Create(
                        $"DeskdropIPC_{Environment.UserName}", 
                        PipeDirection.In, 
                        1, 
                        PipeTransmissionMode.Message, 
                        PipeOptions.Asynchronous, 
                        0, 
                        0, 
                        security);
                        
                    await server.WaitForConnectionAsync();
                    
                    using var reader = new StreamReader(server);
                    var line = await reader.ReadLineAsync();
                    if (!string.IsNullOrEmpty(line))
                    {
                        var parts = line.Split('|');
                        HandleCommandLine(parts, app);
                    }
                }
                catch (Exception ex)
                {
                    LogError(ex);
                    await Task.Delay(1000);
                }
            }
        }

        internal static void LogError(Exception ex)
        {
            try
            {
                var dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory));
                string errorMsg = $"[{DateTime.Now:u}] {ex.GetType().Name}: {ex.Message}\n{ex.StackTrace}";
                var inner = ex.InnerException;
                while (inner != null)
                {
                    errorMsg += $"\n--- Inner Exception: {inner.GetType().Name}: {inner.Message}\n{inner.StackTrace}";
                    inner = inner.InnerException;
                }
                errorMsg += "\n\n";
                File.AppendAllText(Path.Combine(dir, "crash.txt"), errorMsg);
            }
            catch { }
        }

        public class AppSettings
        {
            public bool EnableHotkeys { get; set; } = true;
            public bool ShowNotifications { get; set; } = true;
            public bool SyncEnabled { get; set; } = true;
            public string DeviceName { get; set; } = "";
            public ushort Port { get; set; } = 0;
        }

        public static AppSettings LoadSettings()
        {
            var s = new AppSettings();
            try
            {
                using var key = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(@"Software\Deskdrop");
                if (key != null)
                {
                    s.EnableHotkeys = (int)key.GetValue("EnableHotkeys", 1) == 1;
                    s.ShowNotifications = (int)key.GetValue("ShowNotifications", 1) == 1;
                    s.SyncEnabled = (int)key.GetValue("SyncEnabled", 1) == 1;
                    s.DeviceName = key.GetValue("DeviceName", "") as string ?? "";
                    s.Port = (ushort)(int)key.GetValue("Port", 0);
                }
            }
            catch { }
            return s;
        }

    }
}

