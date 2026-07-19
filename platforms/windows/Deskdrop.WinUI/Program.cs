using Microsoft.UI.Dispatching;
using System;
using System.IO;
using System.IO.Pipes;
using System.Threading;
using System.Threading.Tasks;

namespace Deskdrop.WinUI
{
    internal static class Program
    {
        [STAThread]
        static void Main(string[] args)
        {
            try
            {
                using var mutex = new Mutex(true, "Deskdrop_SingleInstance_v1_" + Environment.UserName, out bool isNew);
                if (!isNew)
                {
                    if (args.Length > 0)
                    {
                        try
                        {
                            using var client = new NamedPipeClientStream(".", "DeskdropIPC_" + Environment.UserName, PipeDirection.Out);
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
                            using var client = new NamedPipeClientStream(".", "DeskdropIPC_" + Environment.UserName, PipeDirection.Out);
                            client.Connect(1000);
                            using var writer = new StreamWriter(client);
                            writer.WriteLine("--open-dashboard");
                            writer.Flush();
                        }
                        catch { }
                    }
                    return;
                }

                global::Microsoft.UI.Xaml.Application.Start((p) => {
                    var context = new global::Microsoft.UI.Dispatching.DispatcherQueueSynchronizationContext(
                        global::Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread());
                    global::System.Threading.SynchronizationContext.SetSynchronizationContext(context);
                    
                    var app = new App();
                });
            }
            catch (Exception ex)
            {
                LogError(ex);
            }
        }

        internal static void LogError(Exception ex)
        {
            try
            {
                var dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory));
                string errorMsg = "[" + DateTime.Now.ToString("u") + "] " + ex.GetType().Name + ": " + ex.Message + "\n" + ex.StackTrace + "\n\n";
                File.AppendAllText(Path.Combine(dir, "real_crash.txt"), errorMsg);
            }
            catch { }
        }
    }
}



