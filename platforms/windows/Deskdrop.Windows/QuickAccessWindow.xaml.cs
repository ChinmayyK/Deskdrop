using System;
using System.ComponentModel;
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
            DeskdropStore.Shared.PropertyChanged += OnStoreChanged;
        }

        private void OnStoreChanged(object? sender, PropertyChangedEventArgs e)
        {
            if (e.PropertyName == nameof(DeskdropStore.History))
            {
                Dispatcher.Invoke(() => {
                    TimelineList.ItemsSource = DeskdropStore.Shared.History;
                });
            }
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
    }
}
