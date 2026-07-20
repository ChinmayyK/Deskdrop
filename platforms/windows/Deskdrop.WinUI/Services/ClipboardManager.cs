using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using System.Collections.ObjectModel;
using System.Collections.Generic;
using Microsoft.UI.Dispatching;
using Windows.ApplicationModel.DataTransfer;
using Deskdrop.WinUI.Views;
using System.Text.Json;
using Microsoft.UI.Xaml;

namespace Deskdrop.WinUI.Services
{
    public class ClipboardManager : IDisposable
    {
        private readonly DispatcherQueue _dispatcher;
        private string _lastText = string.Empty;
        private readonly DispatcherTimer _timer;

        public ObservableCollection<HistoryItem> History { get; set; } = new ObservableCollection<HistoryItem>();
        public event Action<string>? QuickContextUpdated;

        public ClipboardManager()
        {
            _dispatcher = DispatcherQueue.GetForCurrentThread();
            _timer = new DispatcherTimer();
            _timer.Interval = TimeSpan.FromMilliseconds(1000);
            _timer.Tick += OnTick;
            _timer.Start();
        }

        private async void OnTick(object? sender, object e)
        {
            await CheckClipboardAsync();
        }

        private async Task CheckClipboardAsync()
        {
            try
            {
                var packageView = global::Windows.ApplicationModel.DataTransfer.Clipboard.GetContent();
                if (packageView == null) return;
                
                if (packageView.Contains(StandardDataFormats.Text))
                {
                    var text = await packageView.GetTextAsync();
                    if (!string.IsNullOrEmpty(text) && text != _lastText)
                    {
                        _lastText = text;
                        DaemonClient.Send(new { cmd = "clipboard_push", content = text });
                    }
                }
                else if (packageView.Contains(StandardDataFormats.StorageItems))
                {
                    var items = await packageView.GetStorageItemsAsync();
                    foreach (var item in items)
                    {
                        if (item is Windows.Storage.StorageFile file)
                        {
                            try { DaemonClient.SendFilePath(file.Path, file.Name, "application/octet-stream"); } catch { }
                        }
                    }
                }
            }
            catch { }
        }

        public void HandleIncomingData(JsonElement json)
        {
            if (json.TryGetProperty("type", out var typeProp) && typeProp.GetString() == "file")
            {
                string name = json.GetProperty("name").GetString() ?? "Unknown";
                string from = json.GetProperty("from").GetString() ?? "Unknown";
                _dispatcher.TryEnqueue(() => {
                    new IncomingFileBannerWindow(name, from).Activate();
                });
            }
        }

        public void PushFile(string path, string? targetDevice = null)
        {
            if (!string.IsNullOrEmpty(path) && File.Exists(path))
            {
                string name = Path.GetFileName(path);
                try { DaemonClient.SendFilePath(path, name, "application/octet-stream", targetDevice); } catch { }
            }
        }

        public void PushLocalClipboard()
        {
        }

        public void Dispose()
        {
            _timer.Stop();
        }
    }
}

