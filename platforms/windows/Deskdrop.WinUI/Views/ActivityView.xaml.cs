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
            
            DeskdropStore.Shared.PropertyChanged += (s, e) => {
                if (e.PropertyName == nameof(DeskdropStore.Shared.History))
                {
                    DispatcherQueue.TryEnqueue(() => {
                        TimelineList.ItemsSource = DeskdropStore.Shared.History.ToList();
                    });
                }
            };

            TimelineList.ItemsSource = DeskdropStore.Shared.History.ToList();
        }

        private void TxtSearch_TextChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
        {
            var text = sender.Text.ToLower();
            if (string.IsNullOrWhiteSpace(text))
            {
                TimelineList.ItemsSource = DeskdropStore.Shared.History.ToList();
            }
            else
            {
                TimelineList.ItemsSource = DeskdropStore.Shared.History.Where(h => h.display_text.ToLower().Contains(text) || h.path.ToLower().Contains(text)).ToList();
            }
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
            if (sender is Button btn && btn.DataContext is HistoryItem item)
            {
                item.IsPinned = !item.IsPinned;
                TimelineList.ItemsSource = DeskdropStore.Shared.History
                    .OrderByDescending(h => h.IsPinned)
                    .ThenByDescending(h => h.Time)
                    .ToList();
            }
        }

        private void BtnDeleteItem_Click(object sender, RoutedEventArgs e)
        {
            if (sender is Button btn && btn.DataContext is HistoryItem item)
            {
                DeskdropStore.Shared.History.Remove(item);
                App.Clipboard?.History.Remove(item);
                TimelineList.ItemsSource = DeskdropStore.Shared.History.ToList();
            }
        }
    }
}
