using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using System.Linq;

namespace Deskdrop.WinUI.Views
{
    public sealed partial class ActivityView : Page
    {
        private DeskdropStore _mgr;

        public ActivityView()
        {
            this.InitializeComponent();
            DeskdropStore.Shared.PropertyChanged += OnStorePropertyChanged;
            this.Unloaded += (s, e) => {
                try { DeskdropStore.Shared.PropertyChanged -= OnStorePropertyChanged; } catch { }
            };
            try { TimelineList.ItemsSource = DeskdropStore.Shared.History.ToList(); } catch { }
        }

        private void OnStorePropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
        {
            if (e.PropertyName == nameof(DeskdropStore.Shared.History))
            {
                DispatcherQueue?.TryEnqueue(() => {
                    try { TimelineList.ItemsSource = DeskdropStore.Shared.History.ToList(); } catch { }
                });
            }
        }

        private void TxtSearch_TextChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
        {
            try
            {
                var text = sender.Text?.ToLowerInvariant() ?? "";
                if (string.IsNullOrWhiteSpace(text))
                {
                    TimelineList.ItemsSource = DeskdropStore.Shared.History.ToList();
                }
                else
                {
                    var snapshot = DeskdropStore.Shared.History.ToList();
                    TimelineList.ItemsSource = snapshot.Where(h => 
                        (h.display_text?.ToLowerInvariant().Contains(text) == true) || 
                        (h.path?.ToLowerInvariant().Contains(text) == true)).ToList();
                }
            }
            catch { }
        }

        private void HistoryItem_Click(object sender, RoutedEventArgs e)
        {
            if (sender is Button btn && btn.DataContext is HistoryItem item)
            {
                if (item.is_text && !string.IsNullOrEmpty(item.FullText))
                {
                    try {
                        var package = new Windows.ApplicationModel.DataTransfer.DataPackage();
                        package.SetText(item.FullText);
                        Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(package);
                    } catch { }
                }
                else if (!string.IsNullOrEmpty(item.path) && System.IO.File.Exists(item.path))
                {
                    try {
                        System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(item.path) { UseShellExecute = true });
                    } catch { }
                }
            }
        }

        private void BtnPinItem_Click(object sender, RoutedEventArgs e)
        {
            try
            {
                if (sender is Button btn && btn.DataContext is HistoryItem item)
                {
                    item.IsPinned = !item.IsPinned;
                    var snapshot = DeskdropStore.Shared.History.ToList();
                    TimelineList.ItemsSource = snapshot
                        .OrderByDescending(h => h.IsPinned)
                        .ThenByDescending(h => h.Time)
                        .ToList();
                }
            }
            catch { }
        }

        private void BtnDeleteItem_Click(object sender, RoutedEventArgs e)
        {
            try
            {
                if (sender is Button btn && btn.DataContext is HistoryItem item)
                {
                    DeskdropStore.Shared.History.Remove(item);
                    App.Clipboard?.History.Remove(item);
                    TimelineList.ItemsSource = DeskdropStore.Shared.History.ToList();
                }
            }
            catch { }
        }
    }
}
