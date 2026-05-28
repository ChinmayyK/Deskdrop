using System;
using System.ComponentModel;
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Interop;

namespace Deskdrop.Windows
{
    public partial class QuickAccessWindow : Window
    {
        private readonly ClipboardManager _clipboardManager;

        public event EventHandler? DashboardRequested;

        public QuickAccessWindow(ClipboardManager clipboardManager)
        {
            InitializeComponent();
            _clipboardManager = clipboardManager;
            TimelineList.ItemsSource = DeskdropStore.Shared.History;
            if (DeviceTargetsList != null) DeviceTargetsList.ItemsSource = DeskdropStore.Shared.Peers;
            DeskdropStore.Shared.PropertyChanged += OnStoreChanged;
        }

        private void OnStoreChanged(object? sender, PropertyChangedEventArgs e)
        {
            Dispatcher.Invoke(() => {
                if (e.PropertyName == nameof(DeskdropStore.History))
                {
                    if (string.IsNullOrWhiteSpace(TxtSearch.Text))
                        TimelineList.ItemsSource = DeskdropStore.Shared.History;
                }
                else if (e.PropertyName == nameof(DeskdropStore.Peers) && DeviceTargetsList != null)
                {
                    DeviceTargetsList.ItemsSource = DeskdropStore.Shared.Peers;
                }
            });
        }

        private void Window_Loaded(object sender, RoutedEventArgs e)
        {
            var cursorPosition = System.Windows.Forms.Cursor.Position;
            var screen = System.Windows.Forms.Screen.FromPoint(cursorPosition);
            var workArea = screen.WorkingArea;

            var source = PresentationSource.FromVisual(this);
            double scaleX = source?.CompositionTarget?.TransformToDevice.M11 ?? 1.0;
            double scaleY = source?.CompositionTarget?.TransformToDevice.M22 ?? 1.0;

            Left = (workArea.Right / scaleX) - Width - 20;
            Top = (workArea.Bottom / scaleY) - Height - 20;
        }

        private void Window_Deactivated(object sender, EventArgs e)
        {
            Close();
        }

        private void BtnSettings_Click(object sender, RoutedEventArgs e)
        {
            DashboardRequested?.Invoke(this, EventArgs.Empty);
            Close();
        }

        private void BtnPinItem_Click(object sender, RoutedEventArgs e)
        {
            if (((FrameworkElement)sender).DataContext is HistoryItem item)
            {
                item.IsPinned = !item.IsPinned;
                DeskdropStore.Shared.TriggerHistoryUpdate();
            }
        }

        private void BtnDeleteItem_Click(object sender, RoutedEventArgs e)
        {
            if (((FrameworkElement)sender).DataContext is HistoryItem item)
            {
                DeskdropStore.Shared.History.Remove(item);
                DeskdropStore.Shared.TriggerHistoryUpdate();
            }
        }

        private void HistoryItem_Click(object sender, MouseButtonEventArgs e)
        {
            if (((FrameworkElement)sender).DataContext is HistoryItem item)
            {
                // Apply to local clipboard
                if (!string.IsNullOrEmpty(item.FullText))
                {
                    try {
                        System.Windows.Forms.Clipboard.SetText(item.FullText);
                        NotificationHelper.ShowToast("Deskdrop", "Copied to clipboard.");
                    } catch { }
                }
                Close();
            }
        }

        private void TxtSearch_TextChanged(object sender, TextChangedEventArgs e)
        {
            var query = TxtSearch.Text.ToLowerInvariant();
            if (string.IsNullOrWhiteSpace(query))
            {
                TimelineList.ItemsSource = DeskdropStore.Shared.History;
            }
            else
            {
                TimelineList.ItemsSource = DeskdropStore.Shared.History
                    .Where(h => (h.Summary?.ToLowerInvariant().Contains(query) == true) || 
                                (h.Source?.ToLowerInvariant().Contains(query) == true) ||
                                (h.FullText?.ToLowerInvariant().Contains(query) == true))
                    .ToList();
            }
        }

        private void DeviceTarget_Click(object sender, MouseButtonEventArgs e)
        {
            if (((FrameworkElement)sender).DataContext is PeerViewModel peer)
            {
                // Send current clipboard to this specific peer
                System.Threading.Tasks.Task.Run(() => 
                {
                    try 
                    {
                        var clipboardText = "";
                        Dispatcher.Invoke(() => {
                            if (System.Windows.Forms.Clipboard.ContainsText())
                                clipboardText = System.Windows.Forms.Clipboard.GetText();
                        });
                        
                        if (!string.IsNullOrEmpty(clipboardText))
                        {
                            var req = new {
                                cmd = "push_clipboard",
                                target_device = peer.device_id,
                                text = clipboardText
                            };
                            DaemonClient.Send(req);
                            Dispatcher.Invoke(() => NotificationHelper.ShowToast("Deskdrop", $"Clipboard sent to {peer.friendly_name}"));
                        }
                    } 
                    catch (Exception ex)
                    {
                        Dispatcher.Invoke(() => NotificationHelper.ShowToast("Deskdrop", $"Failed to send: {ex.Message}"));
                    }
                });
                Close();
            }
        }
    }
}
