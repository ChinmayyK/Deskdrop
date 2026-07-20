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
                // DeskdropStore.Shared.PushLocalClipboard(); // To be implemented
            }
        }

        private void BtnPinItem_Click(object sender, RoutedEventArgs e)
        {
            if (sender is Button btn && btn.DataContext is HistoryItem item)
            {
                // Pin logic
            }
        }

        private void BtnDeleteItem_Click(object sender, RoutedEventArgs e)
        {
            if (sender is Button btn && btn.DataContext is HistoryItem item)
            {
                DeskdropStore.Shared.History.Remove(item);
                TimelineList.ItemsSource = DeskdropStore.Shared.History.ToList();
            }
        }
    }
}
