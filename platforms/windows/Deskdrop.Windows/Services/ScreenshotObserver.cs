using System;
using System.IO;
using System.Threading.Tasks;

namespace Deskdrop.Windows.Services
{
    public class ScreenshotObserver : IDisposable
    {
        private readonly ClipboardManager _clipboardManager;
        private readonly FileSystemWatcher _desktopWatcher;
        private readonly FileSystemWatcher? _screenshotsWatcher;

        public ScreenshotObserver(ClipboardManager clipboardManager)
        {
            _clipboardManager = clipboardManager;

            string desktopPath = Environment.GetFolderPath(Environment.SpecialFolder.Desktop);
            string picturesPath = Environment.GetFolderPath(Environment.SpecialFolder.MyPictures);
            string screenshotsPath = Path.Combine(picturesPath, "Screenshots");

            _desktopWatcher = SetupWatcher(desktopPath);
            if (Directory.Exists(screenshotsPath))
            {
                _screenshotsWatcher = SetupWatcher(screenshotsPath);
            }
        }

        private FileSystemWatcher SetupWatcher(string path)
        {
            var watcher = new FileSystemWatcher(path)
            {
                NotifyFilter = NotifyFilters.FileName | NotifyFilters.LastWrite | NotifyFilters.CreationTime,
                Filter = "*.*",
                EnableRaisingEvents = true
            };
            watcher.Created += OnFileCreated;
            return watcher;
        }

        private void OnFileCreated(object sender, FileSystemEventArgs e)
        {
            string ext = Path.GetExtension(e.FullPath).ToLower();
            if (ext == ".png" || ext == ".jpg" || ext == ".jpeg")
            {
                bool isScreenshotFolder = e.FullPath.Contains("Screenshots", StringComparison.OrdinalIgnoreCase);
                string name = Path.GetFileNameWithoutExtension(e.FullPath).ToLower();
                
                // If it's in the screenshots folder, we assume any new image is a screenshot.
                // If it's on the desktop, we enforce that the filename must contain "screenshot".
                if (isScreenshotFolder || name.Contains("screenshot") || name.Contains("screen shot"))
                {
                    // Delay slightly to ensure the screenshot is completely written to disk by the OS
                    Task.Delay(1000).ContinueWith(_ =>
                    {
                        try
                        {
                            if (File.Exists(e.FullPath))
                            {
                                // Sync the screenshot to the connected phone
                                _clipboardManager.PushFile(e.FullPath);
                                
                                System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                                    NotificationHelper.ShowToast("Screenshot Synced", "Sent screenshot to your device.");
                                });
                            }
                        }
                        catch { } 
                    });
                }
            }
        }

        public void Dispose()
        {
            if (_desktopWatcher != null)
            {
                _desktopWatcher.EnableRaisingEvents = false;
                _desktopWatcher.Dispose();
            }
            if (_screenshotsWatcher != null)
            {
                _screenshotsWatcher.EnableRaisingEvents = false;
                _screenshotsWatcher.Dispose();
            }
        }
    }
}
