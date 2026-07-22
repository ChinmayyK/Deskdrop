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
            mgr.ActivityFeed.CollectionChanged += (s, e) => UpdateFilter();
            UpdateFilter();
        }

        private void UpdateFilter()
        {
            var query = SearchBox?.Text?.Trim() ?? "";
            var items = mgr.ActivityFeed.AsEnumerable();

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

        private void OnSearchTextChanged(object sender, TextChangedEventArgs e)
        {
            UpdateFilter();
        }

        private void OnFilterChipClicked(object sender, RoutedEventArgs e)
        {
            if (sender is Button btn && btn.Content is string tag)
            {
                _activeFilter = tag;
                
                // Update chip styling
                ChipAll.Background = new SolidColorBrush(_activeFilter == "All" ? Microsoft.UI.Colors.DodgerBlue : Microsoft.UI.Colors.Transparent);
                ChipAll.Foreground = new SolidColorBrush(_activeFilter == "All" ? Microsoft.UI.Colors.White : Microsoft.UI.Colors.Black);
                
                ChipText.Background = new SolidColorBrush(_activeFilter == "Text" ? Microsoft.UI.Colors.DodgerBlue : Microsoft.UI.Colors.Transparent);
                ChipText.Foreground = new SolidColorBrush(_activeFilter == "Text" ? Microsoft.UI.Colors.White : Microsoft.UI.Colors.Black);
                
                ChipImage.Background = new SolidColorBrush(_activeFilter == "Image" ? Microsoft.UI.Colors.DodgerBlue : Microsoft.UI.Colors.Transparent);
                ChipImage.Foreground = new SolidColorBrush(_activeFilter == "Image" ? Microsoft.UI.Colors.White : Microsoft.UI.Colors.Black);
                
                ChipFile.Background = new SolidColorBrush(_activeFilter == "File" ? Microsoft.UI.Colors.DodgerBlue : Microsoft.UI.Colors.Transparent);
                ChipFile.Foreground = new SolidColorBrush(_activeFilter == "File" ? Microsoft.UI.Colors.White : Microsoft.UI.Colors.Black);

                UpdateFilter();
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
