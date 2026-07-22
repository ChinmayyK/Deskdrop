using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using System;
using System.Collections.ObjectModel;
using System.Text.Json;
using System.Threading.Tasks;

namespace Deskdrop.WinUI.Views
{
    public sealed partial class RemoteExplorerView : Page
    {
        public DeskdropStore mgr => DeskdropStore.Shared;
        public ObservableCollection<RemoteFile> RemoteFiles { get; } = new ObservableCollection<RemoteFile>();
        private string _currentPath = "/";

        public RemoteExplorerView()
        {
            this.InitializeComponent();
            this.Loaded += (s, e) => LoadRemoteDirectory("/");
        }

        private async void LoadRemoteDirectory(string path)
        {
            if (string.IsNullOrEmpty(path)) path = "/";
            _currentPath = path;
            if (PathBox != null) PathBox.Text = _currentPath;

            var peer = mgr.SelectedPeer;
            if (peer == null || string.IsNullOrEmpty(peer.device_id)) return;

            try
            {
                string? category = null;
                string? source = null;
                if (_currentPath.StartsWith("/category/", StringComparison.OrdinalIgnoreCase))
                    category = _currentPath.Substring("/category/".Length);
                else if (_currentPath.StartsWith("/source/", StringComparison.OrdinalIgnoreCase))
                    source = _currentPath.Substring("/source/".Length);

                var doc = await DaemonClient.RemoteFilesQueryAsync(peer.device_id, summaryOnly: false, category: category, source: source);
                if (doc != null && doc.RootElement.ValueKind != JsonValueKind.Null)
                {
                    var options = new JsonSerializerOptions { PropertyNameCaseInsensitive = true };
                    RemoteFileListResponse? resp = null;
                    if (doc.RootElement.TryGetProperty("files", out _))
                    {
                        resp = JsonSerializer.Deserialize<RemoteFileListResponse>(doc.RootElement.GetRawText(), options);
                    }
                    else if (doc.RootElement.TryGetProperty("data", out var dataEl) && dataEl.TryGetProperty("files", out _))
                    {
                        resp = JsonSerializer.Deserialize<RemoteFileListResponse>(dataEl.GetRawText(), options);
                    }
                    
                    RemoteFiles.Clear();
                    if (resp != null && resp.files != null)
                    {
                        foreach (var f in resp.files)
                        {
                            RemoteFiles.Add(f);
                        }
                    }
                }
            }
            catch (Exception)
            {
                // Handle network or serialization errors gracefully
            }
        }

        private void OnBackClicked(object sender, RoutedEventArgs e)
        {
            if (_currentPath == "/" || string.IsNullOrEmpty(_currentPath))
            {
                DashboardWindow.Current?.NavigateTo("Devices");
                return;
            }

            var trimmed = _currentPath.TrimEnd('/');
            var lastSlash = trimmed.LastIndexOf('/');
            if (lastSlash <= 0)
            {
                LoadRemoteDirectory("/");
            }
            else
            {
                LoadRemoteDirectory(trimmed.Substring(0, lastSlash));
            }
        }

        private void OnPathKeyDown(object sender, KeyRoutedEventArgs e)
        {
            if (e.Key == Windows.System.VirtualKey.Enter && PathBox != null)
            {
                LoadRemoteDirectory(PathBox.Text.Trim());
            }
        }

        private void OnGoClicked(object sender, RoutedEventArgs e)
        {
            if (PathBox != null)
            {
                LoadRemoteDirectory(PathBox.Text.Trim());
            }
        }

        private void OnSendFilesClicked(object sender, RoutedEventArgs e)
        {
            var peer = mgr.SelectedPeer;
            if (peer != null)
            {
                mgr.PickAndSendFiles(peer.device_id);
            }
        }

        private void OnShortcutRootClicked(object sender, RoutedEventArgs e) => LoadRemoteDirectory("/");
        private void OnShortcutDCIMClicked(object sender, RoutedEventArgs e) => LoadRemoteDirectory("/DCIM/Camera");
        private void OnShortcutPicturesClicked(object sender, RoutedEventArgs e) => LoadRemoteDirectory("/Pictures");
        private void OnShortcutDownloadsClicked(object sender, RoutedEventArgs e) => LoadRemoteDirectory("/Download");
        private void OnShortcutDocumentsClicked(object sender, RoutedEventArgs e) => LoadRemoteDirectory("/Documents");

        private void OnItemClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is RemoteFile item)
            {
                if (item.is_dir)
                {
                    var nextPath = _currentPath.TrimEnd('/') + "/" + item.name;
                    LoadRemoteDirectory(nextPath);
                }
            }
        }

        private void OnDownloadClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is RemoteFile item)
            {
                var peer = mgr.SelectedPeer;
                if (peer != null)
                {
                    if (item.file_id > 0)
                        DaemonClient.RemoteFilePullRequest(peer.device_id, item.file_id);
                    else if (ulong.TryParse(item.id, out var fid))
                        DaemonClient.RemoteFilePullRequest(peer.device_id, fid);
                }
            }
        }
    }
}
