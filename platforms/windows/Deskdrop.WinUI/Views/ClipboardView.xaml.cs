using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using System;
using System.Collections.ObjectModel;
using System.Linq;

namespace Deskdrop.WinUI.Views
{
    public sealed partial class ClipboardView : Page
    {
        public DeskdropStore mgr => DeskdropStore.Shared;
        public ObservableCollection<ActivityEntry> FilteredFeed { get; } = new ObservableCollection<ActivityEntry>();
        private string _activeFilter = "All";

        public ClipboardView()
        {
            this.InitializeComponent();
            mgr.ActivityFeed.CollectionChanged += OnActivityFeedChanged;
            this.Unloaded += (s, e) => {
                try { mgr.ActivityFeed.CollectionChanged -= OnActivityFeedChanged; } catch (Exception ex) { App.HandleError(ex); }
            };
            UpdateFilter();
        }

        private void OnActivityFeedChanged(object? sender, System.Collections.Specialized.NotifyCollectionChangedEventArgs e)
        {
            UpdateFilter();
        }

        private void UpdateFilter()
        {
            try
            {
                var query = SearchBox?.Text?.Trim() ?? "";
                var snapshot = mgr.ActivityFeed.ToList();
                var items = snapshot.AsEnumerable();

                if (!string.IsNullOrEmpty(query))
                {
                    items = items.Where(i => (i.Title?.Contains(query, StringComparison.OrdinalIgnoreCase) ?? false) ||
                                             (i.Source?.Contains(query, StringComparison.OrdinalIgnoreCase) ?? false) ||
                                             (i.TypeLabel?.Contains(query, StringComparison.OrdinalIgnoreCase) ?? false));
                }

                if (_activeFilter != "All")
                {
                    items = items.Where(i => string.Equals(i.TypeLabel, _activeFilter, StringComparison.OrdinalIgnoreCase) ||
                                             (_activeFilter == "File" && (i.TypeLabel == "File" || i.TypeLabel == "Applied")) ||
                                             (_activeFilter == "Text" && i.TypeLabel == "Clipboard"));
                }

                FilteredFeed.Clear();
                foreach (var item in items)
                {
                    FilteredFeed.Add(item);
                }
            }
            catch (Exception ex) { App.HandleError(ex); }
        }

        private void OnSearchTextChanged(object sender, TextChangedEventArgs e)
        {
            UpdateFilter();
        }

        private void OnFilterChipClicked(object sender, RoutedEventArgs e)
        {
            if (sender is Button btn && btn.Content is string tag)
            {
                _activeFilter = tag;
                
                var accentBrush = (Brush)Application.Current.Resources["AppAccentBrush"];
                var surfaceBrush = (Brush)Application.Current.Resources["AppSurfaceBrush"];
                var primaryText = (Brush)Application.Current.Resources["TextFillColorPrimaryBrush"];
                var secondaryText = (Brush)Application.Current.Resources["TextFillColorSecondaryBrush"];

                ChipAll.Background = _activeFilter == "All" ? accentBrush : surfaceBrush;
                ChipAll.Foreground = _activeFilter == "All" ? new SolidColorBrush(Microsoft.UI.Colors.White) : secondaryText;
                
                ChipText.Background = _activeFilter == "Text" ? accentBrush : surfaceBrush;
                ChipText.Foreground = _activeFilter == "Text" ? new SolidColorBrush(Microsoft.UI.Colors.White) : secondaryText;
                
                ChipImage.Background = _activeFilter == "Image" ? accentBrush : surfaceBrush;
                ChipImage.Foreground = _activeFilter == "Image" ? new SolidColorBrush(Microsoft.UI.Colors.White) : secondaryText;
                
                ChipFile.Background = _activeFilter == "File" ? accentBrush : surfaceBrush;
                ChipFile.Foreground = _activeFilter == "File" ? new SolidColorBrush(Microsoft.UI.Colors.White) : secondaryText;

                UpdateFilter();
            }
        }

        private async void OnPushLocalClipboardClicked(object sender, RoutedEventArgs e)
        {
            try
            {
                var dataPackageView = Windows.ApplicationModel.DataTransfer.Clipboard.GetContent();
                if (dataPackageView.Contains(Windows.ApplicationModel.DataTransfer.StandardDataFormats.Text))
                {
                    var text = await dataPackageView.GetTextAsync();
                    if (!string.IsNullOrEmpty(text))
                    {
                        var firstConnected = mgr.ConnectedPeers.FirstOrDefault();
                        if (firstConnected != null)
                        {
                            mgr.SendPushText(text, firstConnected.device_id);
                        }
                    }
                }
            }
            catch (Exception ex)
            {
                App.HandleError(ex);
            }
        }

        private void OnCopyActivityTextClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is ActivityEntry entry)
            {
                var text = !string.IsNullOrEmpty(entry.text_preview) ? entry.text_preview : entry.Title;
                if (!string.IsNullOrEmpty(text))
                {
                    var dp = new Windows.ApplicationModel.DataTransfer.DataPackage();
                    dp.SetText(text);
                    Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(dp);
                }
            }
        }

        private void OnApplyPendingClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is PendingClipboard pending && !string.IsNullOrEmpty(pending.content_hash))
            {
                mgr.ApplyClipboardItem(pending.content_hash);
            }
        }

        private void OnApplyActivityClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is ActivityEntry entry && !string.IsNullOrEmpty(entry.content_hash))
            {
                mgr.ApplyClipboardItem(entry.content_hash);
            }
        }
    }
}
