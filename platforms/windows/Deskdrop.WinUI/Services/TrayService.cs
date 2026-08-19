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

        public TrayService()
        {
            Current = this;
            EnsureTrayProcessRunning();
        }

        public TrayService(IntPtr unusedHwnd) : this()
        {
        }

        private void EnsureTrayProcessRunning()
        {
            try
            {
                var procs = Process.GetProcessesByName("Deskdrop.Tray");
                if (procs.Length == 0)
                {
                    var baseDir = AppContext.BaseDirectory;
                    var trayExe = Path.Combine(baseDir, "Deskdrop.Tray.exe");
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
        }
    }
}
