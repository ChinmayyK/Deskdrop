using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using System;
using System.Collections.ObjectModel;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

namespace Deskdrop.WinUI.Views
{
    public sealed partial class RemoteExplorerView : Page
    {
        public DeskdropStore mgr => DeskdropStore.Shared;
        public ObservableCollection<RemoteFile> RemoteFiles { get; } = new ObservableCollection<RemoteFile>();

        // Breadcrumb trail for the location bar. Index 0 is always the root,
        // labelled for humans rather than shown as "/".
        public ObservableCollection<string> PathSegments { get; } = new ObservableCollection<string> { RootSegmentLabel };
        private const string RootSegmentLabel = "All files";

        private string _currentPath = "/";
        // Bumped at the start of every LoadRemoteDirectory call and captured
        // as a ticket; a completion whose ticket no longer matches means a
        // newer navigation has since started, so its result is discarded
        // instead of clobbering the UI with a stale directory listing.
        private int _loadGeneration = 0;
        private static readonly System.Collections.Generic.Dictionary<string, JsonDocument> _cache = new();
        private static readonly System.Collections.Generic.Dictionary<string, Microsoft.UI.Xaml.Media.Imaging.BitmapImage> _thumbnailCache = new();

        // Each thumbnail is a peer-to-peer round trip (the daemon asks the
        // Android device to generate/send one, up to a 10s engine-side
        // timeout) over what's effectively a single-instance named pipe.
        // Scrolling a 100-item list can trigger a dozen fetches at once;
        // without throttling, most of them queue behind each other and blow
        // past their own client-side timeout, so only the first couple ever
        // resolve. Cap concurrency and retry once for anything that fails
        // under that pressure.
        private static readonly SemaphoreSlim _thumbnailThrottle = new(2, 2);

        public RemoteExplorerView()
        {
            this.InitializeComponent();
            this.Loaded += (s, e) => LoadRemoteDirectory("/");
        }

        private async System.Threading.Tasks.Task LoadRemoteDirectory(string path, bool forceRefresh = false)
        {
            var myGeneration = ++_loadGeneration;
            if (string.IsNullOrEmpty(path)) path = "/";
            _currentPath = path;
            if (PathBox != null) PathBox.Text = _currentPath;
            UpdatePathSegments();

            var peer = mgr.SelectedPeer;
            if (peer == null || string.IsNullOrEmpty(peer.device_id))
            {
                UpdateEmptyStates();
                return;
            }

            try
            {
                string? category = null;
                string? source = null;
                if (_currentPath.StartsWith("/category/", StringComparison.OrdinalIgnoreCase))
                    category = _currentPath.Substring("/category/".Length);
                else if (_currentPath.StartsWith("/source/", StringComparison.OrdinalIgnoreCase))
                    source = _currentPath.Substring("/source/".Length);

                string cacheKey = $"{peer.device_id}_{_currentPath}";
                JsonDocument? doc = null;

                if (!forceRefresh && _cache.TryGetValue(cacheKey, out var cachedDoc))
                {
                    doc = cachedDoc;
                }
                else
                {
                    doc = await DaemonClient.RemoteFilesQueryAsync(peer.device_id, summaryOnly: false, category: category, source: source);
                    if (doc != null)
                    {
                        _cache[cacheKey] = doc;
                    }
                }

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
                    
                    App.MainWindow?.DispatcherQueue?.TryEnqueue(() =>
                    {
                        if (myGeneration != _loadGeneration) return;
                        RemoteFiles.Clear();
                        if (resp != null && resp.files != null)
                        {
                            foreach (var f in resp.files)
                            {
                                RemoteFiles.Add(f);
                            }
                        }
                        UpdateEmptyStates();
                    });
                }
            }
            catch (Exception)
            {
                // Handle network or serialization errors gracefully
            }
        }

        // Rebuilds the breadcrumb from the current path. Kept as a plain
        // rebuild rather than a diff: the trail is at most a handful of
        // items, and correctness beats cleverness here.
        private void UpdatePathSegments()
        {
            try
            {
                PathSegments.Clear();
                PathSegments.Add(RootSegmentLabel);

                foreach (var segment in (_currentPath ?? "/").Split('/', StringSplitOptions.RemoveEmptyEntries))
                {
                    PathSegments.Add(segment);
                }
            }
            catch (Exception ex) { App.HandleError(ex); }
        }

        private void OnBreadcrumbItemClicked(Microsoft.UI.Xaml.Controls.BreadcrumbBar sender,
                                             Microsoft.UI.Xaml.Controls.BreadcrumbBarItemClickedEventArgs args)
        {
            try
            {
                // Index 0 is the synthetic root label; anything beyond it maps
                // back onto the real path segments.
                if (args.Index <= 0)
                {
                    _ = LoadRemoteDirectory("/");
                    return;
                }

                var segments = PathSegments.Skip(1).Take(args.Index).ToArray();
                _ = LoadRemoteDirectory("/" + string.Join("/", segments));
            }
            catch (Exception ex) { App.HandleError(ex); }
        }

        // "No device" and "empty folder" are different problems with
        // different fixes, so they get different empty states instead of one
        // message that is wrong half the time.
        private void UpdateEmptyStates()
        {
            try
            {
                var hasPeer = mgr.SelectedPeer != null && !string.IsNullOrEmpty(mgr.SelectedPeer.device_id);
                var hasFiles = RemoteFiles.Count > 0;

                ItemCountText.Text = RemoteFiles.Count == 1 ? "1 item" : $"{RemoteFiles.Count} items";

                NoDeviceState.Visibility = hasPeer ? Visibility.Collapsed : Visibility.Visible;
                EmptyFolderState.Visibility = (hasPeer && !hasFiles) ? Visibility.Visible : Visibility.Collapsed;
                FileList.Visibility = hasFiles ? Visibility.Visible : Visibility.Collapsed;
            }
            catch (Exception ex) { App.HandleError(ex); }
        }

        private void OnGoToDevicesClicked(object sender, RoutedEventArgs e)
        {
            DashboardWindow.Current?.NavigateTo("Devices");
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
                _ = LoadRemoteDirectory("/");
            }
            else
            {
                _ = LoadRemoteDirectory(trimmed.Substring(0, lastSlash));
            }
        }

        private void OnPathKeyDown(object sender, KeyRoutedEventArgs e)
        {
            if (e.Key == Windows.System.VirtualKey.Enter && PathBox != null)
            {
                _ = LoadRemoteDirectory(PathBox.Text.Trim());
            }
        }

        private void OnGoClicked(object sender, RoutedEventArgs e)
        {
            if (PathBox != null)
            {
                _ = LoadRemoteDirectory(PathBox.Text.Trim());
            }
        }

        private void OnSendFilesClicked(object sender, RoutedEventArgs e)
        {
            var peer = mgr.SelectedPeer;
            if (peer != null)
            {
                _ = mgr.PickAndSendFiles(peer.device_id);
            }
        }

        private void OnRefreshClicked(object sender, RoutedEventArgs e)
        {
            _ = LoadRemoteDirectory(_currentPath, true);
        }

        private void OnShortcutRootClicked(object sender, RoutedEventArgs e) => _ = LoadRemoteDirectory("/");
        private void OnShortcutDCIMClicked(object sender, RoutedEventArgs e) => _ = LoadRemoteDirectory("/DCIM/Camera");
        private void OnShortcutPicturesClicked(object sender, RoutedEventArgs e) => _ = LoadRemoteDirectory("/Pictures");
        private void OnShortcutDownloadsClicked(object sender, RoutedEventArgs e) => _ = LoadRemoteDirectory("/Download");
        private void OnShortcutDocumentsClicked(object sender, RoutedEventArgs e) => _ = LoadRemoteDirectory("/Documents");
        private void OnShortcutMusicClicked(object sender, RoutedEventArgs e) => _ = LoadRemoteDirectory("/Music");
        private void OnShortcutMoviesClicked(object sender, RoutedEventArgs e) => _ = LoadRemoteDirectory("/Movies");

        private void OnItemClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is RemoteFile item)
            {
                OpenIfDirectory(item);
            }
        }

        // Double-click to open, matching File Explorer. Single-click used to
        // navigate, which made it impossible to hover a row without being
        // taken somewhere - the explicit Open button and the context menu
        // cover the same ground for anyone who prefers them.
        private void OnFileRowDoubleTapped(object sender, DoubleTappedRoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is RemoteFile item)
            {
                OpenIfDirectory(item);
            }
        }

        private void OpenIfDirectory(RemoteFile item)
        {
            if (!item.is_dir) return;
            var nextPath = _currentPath.TrimEnd('/') + "/" + item.name;
            _ = LoadRemoteDirectory(nextPath);
        }

        private async void OnDownloadClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is RemoteFile item)
            {
                var peer = mgr.SelectedPeer;
                if (peer != null)
                {
                    ulong fileId = item.file_id > 0 ? item.file_id : (ulong.TryParse(item.id, out var fid) ? fid : 0);
                    if (fileId > 0)
                    {
                        var resp = await Task.Run(() => DaemonClient.RemoteFilePullRequest(peer.device_id, fileId));
                        DaemonActions.ReportIfFailed("Download", resp);
                    }
                }
            }
        }

        private static ulong ResolveFileId(RemoteFile item) =>
            item.file_id > 0 ? item.file_id : (ulong.TryParse(item.id, out var fid) ? fid : 0);

        private async void OnRenameRemoteFileClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is not RemoteFile item) return;
            var peer = mgr.SelectedPeer;
            var fileId = ResolveFileId(item);
            if (peer == null || fileId == 0) return;

            var input = new TextBox { Text = item.name, SelectionStart = 0, SelectionLength = item.name.Length };
            var dialog = new ContentDialog
            {
                Title = "Rename",
                Content = input,
                PrimaryButtonText = "Rename",
                CloseButtonText = "Cancel",
                DefaultButton = ContentDialogButton.Primary,
                XamlRoot = this.XamlRoot,
            };

            var result = await dialog.ShowAsync();
            if (result != ContentDialogResult.Primary) return;

            var newName = input.Text?.Trim();
            if (string.IsNullOrEmpty(newName) || newName == item.name) return;

            try
            {
                var resp = await Task.Run(() => DaemonClient.RemoteFileActionRequest(peer.device_id, fileId, "rename", newName));
                DaemonActions.ReportIfFailed("Rename", resp);
                await LoadRemoteDirectory(_currentPath, forceRefresh: true);
            }
            catch (Exception ex) { App.HandleError(ex); }
        }

        private async void OnDeleteRemoteFileClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is not RemoteFile item) return;
            var peer = mgr.SelectedPeer;
            var fileId = ResolveFileId(item);
            if (peer == null || fileId == 0) return;

            var dialog = new ContentDialog
            {
                Title = "Delete this item?",
                Content = $"\"{item.name}\" will be permanently deleted from {peer.DisplayName}. This can't be undone.",
                PrimaryButtonText = "Delete",
                CloseButtonText = "Cancel",
                DefaultButton = ContentDialogButton.Close,
                XamlRoot = this.XamlRoot,
            };

            var result = await dialog.ShowAsync();
            if (result != ContentDialogResult.Primary) return;

            try
            {
                var resp = await Task.Run(() => DaemonClient.RemoteFileActionRequest(peer.device_id, fileId, "delete"));
                DaemonActions.ReportIfFailed("Delete", resp);
                RemoteFiles.Remove(item);
                await LoadRemoteDirectory(_currentPath, forceRefresh: true);
            }
            catch (Exception ex) { App.HandleError(ex); }
        }

        // Fires as rows scroll into view (and on recycle) - only kick off a
        // thumbnail fetch for image/video files that don't have one yet,
        // mirroring macOS's onAppear-triggered requestRemoteThumbnail.
        private void OnFileContainerContentChanging(ListViewBase sender, ContainerContentChangingEventArgs args)
        {
            if (args.Phase != 0) return;
            if (args.Item is not RemoteFile file) return;
            if (!file.IsPreviewable || file.HasThumbnail || file.ThumbnailRequested) return;

            var peer = mgr.SelectedPeer;
            var fileId = file.file_id > 0 ? file.file_id : (ulong.TryParse(file.id, out var pid) ? pid : 0);
            var cacheKey = $"{peer?.device_id}_{fileId}";
            if (peer != null && _thumbnailCache.TryGetValue(cacheKey, out var cached))
            {
                file.ThumbnailRequested = true;
                file.Thumbnail = cached;
                return;
            }

            file.ThumbnailRequested = true;
            _ = FetchThumbnailAsync(file, fileId, cacheKey);
        }

        private async Task FetchThumbnailAsync(RemoteFile file, ulong fileId, string cacheKey)
        {
            var peer = mgr.SelectedPeer;
            if (peer == null || string.IsNullOrEmpty(peer.device_id) || fileId == 0) return;

            string? base64 = null;
            const int maxAttempts = 2;
            for (var attempt = 1; attempt <= maxAttempts && base64 == null; attempt++)
            {
                await _thumbnailThrottle.WaitAsync();
                JsonDocument? doc;
                try
                {
                    doc = await DaemonClient.RemoteThumbnailRequestAsync(peer.device_id, fileId, 160);
                }
                catch (Exception ex) { App.HandleError(ex); doc = null; }
                finally { _thumbnailThrottle.Release(); }

                if (doc == null)
                {
                    if (attempt < maxAttempts) await Task.Delay(400);
                    continue;
                }

                try
                {
                    var root = doc.RootElement;
                    var dataEl = root.TryGetProperty("data", out var d) ? d : root;
                    if (dataEl.TryGetProperty("data_base64", out var b64El) && b64El.ValueKind == JsonValueKind.String)
                        base64 = b64El.GetString();
                }
                catch (Exception ex) { App.HandleError(ex); }

                if (string.IsNullOrEmpty(base64) && attempt < maxAttempts) await Task.Delay(400);
            }

            if (string.IsNullOrEmpty(base64))
            {
                // Let a later scroll-into-view try again instead of giving up
                // on this file for the rest of the session.
                file.ThumbnailRequested = false;
                return;
            }

            App.MainWindow?.DispatcherQueue?.TryEnqueue(async () =>
            {
                try
                {
                    var bytes = Convert.FromBase64String(base64);
                    using var stream = new Windows.Storage.Streams.InMemoryRandomAccessStream();
                    using var writer = new Windows.Storage.Streams.DataWriter(stream.GetOutputStreamAt(0));
                    writer.WriteBytes(bytes);
                    await writer.StoreAsync();

                    var bitmap = new Microsoft.UI.Xaml.Media.Imaging.BitmapImage();
                    await bitmap.SetSourceAsync(stream);
                    _thumbnailCache[cacheKey] = bitmap;
                    file.Thumbnail = bitmap;
                }
                catch (Exception ex) { App.HandleError(ex); }
            });
        }
    }
}
