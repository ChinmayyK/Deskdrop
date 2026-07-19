using System;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using System.Linq;

namespace Deskdrop.Windows.Services
{
    public class SystemTelemetryPoller : IDisposable
    {
        private CancellationTokenSource? _cts;
        private Task? _pollerTask;

        public SystemTelemetryPoller()
        {
        }

        public void Start()
        {
            _cts = new CancellationTokenSource();
            _pollerTask = Task.Run(() => PollLoop(_cts.Token), _cts.Token);
        }

        public void Stop()
        {
            if (_cts != null)
            {
                _cts.Cancel();
                _cts.Dispose();
                _cts = null;
            }
        }

        public void Dispose()
        {
            Stop();
        }

        private async Task PollLoop(CancellationToken token)
        {
            while (!token.IsCancellationRequested)
            {
                try
                {
                    PushBatteryStatus();
                    PushStorageStatus();
                }
                catch (Exception ex)
                {
                    Console.WriteLine($"Error polling telemetry: {ex.Message}");
                }
                
                // Poll every 60 seconds
                await Task.Delay(TimeSpan.FromSeconds(60), token);
            }
        }

        private void PushBatteryStatus()
        {
            try
            {
                var powerStatus = System.Windows.Forms.SystemInformation.PowerStatus;
                int level = (int)(powerStatus.BatteryLifePercent * 100);
                bool charging = powerStatus.PowerLineStatus == System.Windows.Forms.PowerLineStatus.Online;
                
                if (level >= 0 && level <= 100)
                {
                    DaemonClient.PushBatteryStatus(level, charging);
                }
            }
            catch { /* Ignore error on desktop machines without battery */ }
        }

        private void PushStorageStatus()
        {
            try
            {
                var fixedDrives = DriveInfo.GetDrives().Where(d => d.DriveType == DriveType.Fixed && d.IsReady);
                
                ulong totalBytes = 0;
                ulong freeBytes = 0;

                foreach (var drive in fixedDrives)
                {
                    totalBytes += (ulong)drive.TotalSize;
                    freeBytes += (ulong)drive.TotalFreeSpace;
                }

                // Since we don't have quick exact metrics for Images/Videos/Apps on Windows, we send approximated or zeroed categories.
                // The main remote explorer will display total space correctly.
                DaemonClient.PushStorageStatus(0, 0, 0, freeBytes, totalBytes);
            }
            catch { /* Ignore errors */ }
        }
    }
}
