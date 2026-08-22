using System;
using System.Diagnostics;
using System.IO;
using System.IO.Pipes;
using Microsoft.UI.Xaml;

namespace Deskdrop.WinUI.Services
{
    public class TrayService : IDisposable
    {
        public static TrayService? Current { get; private set; }

        private readonly System.Threading.Timer _watchdogTimer;
        private int _consecutiveIpcFailures = 0;

        public TrayService()
        {
            Current = this;
            EnsureTrayProcessRunning();

            // The tray helper must never stay dead or hung - poll its
            // liveness (process exists) and responsiveness (IPC pipe
            // accepts a connection) and respawn it if either check fails.
            _watchdogTimer = new System.Threading.Timer(
                _ => CheckTrayHealth(),
                null,
                TimeSpan.FromSeconds(20),
                TimeSpan.FromSeconds(20));
        }

        public TrayService(IntPtr unusedHwnd) : this()
        {
        }

        private void CheckTrayHealth()
        {
            try
            {
                var procs = Process.GetProcessesByName("Deskdrop.Tray");
                if (procs.Length == 0)
                {
                    _consecutiveIpcFailures = 0;
                    EnsureTrayProcessRunning();
                    return;
                }

                if (IsIpcResponsive())
                {
                    _consecutiveIpcFailures = 0;
                    return;
                }

                _consecutiveIpcFailures++;
                // Require a couple of consecutive failures before acting -
                // a single missed connect can just be transient contention.
                if (_consecutiveIpcFailures >= 2)
                {
                    _consecutiveIpcFailures = 0;
                    foreach (var proc in procs)
                    {
                        try { proc.Kill(); } catch { }
                    }
                    EnsureTrayProcessRunning();
                }
            }
            catch (Exception ex) { App.HandleError(ex); }
        }

        private bool IsIpcResponsive()
        {
            try
            {
                using var client = new NamedPipeClientStream(".", "Deskdrop_Tray_Pipe", PipeDirection.Out);
                client.Connect(500);
                return true;
            }
            catch { return false; }
        }

        private void EnsureTrayProcessRunning()
        {
            try
            {
                var procs = Process.GetProcessesByName("Deskdrop.Tray");
                if (procs.Length == 0)
                {
                    var baseDir = AppContext.BaseDirectory;
                    // Tray lives in its own subdirectory to avoid DLL version conflicts
                    // (WinUI uses System.Drawing.Common v9.x, Tray needs v8.x for WinForms)
                    var trayExe = Path.Combine(baseDir, "tray", "Deskdrop.Tray.exe");
                    if (!File.Exists(trayExe))
                    {
                        // Fallback: same directory (dev/debug scenario)
                        trayExe = Path.Combine(baseDir, "Deskdrop.Tray.exe");
                    }
                    if (File.Exists(trayExe))
                    {
                        Process.Start(new ProcessStartInfo
                        {
                            FileName = trayExe,
                            UseShellExecute = true
                        });
                    }
                }
            }
            catch (Exception ex)
            {
                App.HandleError(ex);
            }
        }

        public void UpdateTooltip(string text)
        {
            SendIpcMessage("TIP:" + text);
        }

        public void ShowNotification(string title, string text)
        {
            SendIpcMessage("NOTIFY:" + text);
        }

        private void SendIpcMessage(string message)
        {
            System.Threading.Tasks.Task.Run(() =>
            {
                try
                {
                    using var client = new NamedPipeClientStream(".", "Deskdrop_Tray_Pipe", PipeDirection.Out);
                    client.Connect(200);
                    using var writer = new StreamWriter(client);
                    writer.WriteLine(message);
                    writer.Flush();
                }
                catch { }
            });
        }

        public void Dispose()
        {
            try { _watchdogTimer?.Dispose(); } catch { }

            try
            {
                // Ask the tray process to exit gracefully first (synchronously,
                // so we don't race the fallback Kill() below against an
                // async fire-and-forget send).
                try
                {
                    using var client = new NamedPipeClientStream(".", "Deskdrop_Tray_Pipe", PipeDirection.Out);
                    client.Connect(200);
                    using var writer = new StreamWriter(client);
                    writer.WriteLine("QUIT");
                    writer.Flush();
                }
                catch { }

                foreach (var proc in Process.GetProcessesByName("Deskdrop.Tray"))
                {
                    try
                    {
                        if (!proc.WaitForExit(500))
                        {
                            proc.Kill();
                        }
                    }
                    catch { }
                }
            }
            catch (Exception ex) { App.HandleError(ex); }
        }
    }
}
