using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Windowing;
using Microsoft.UI.Composition.SystemBackdrops;
using WinRT.Interop;
using System.Collections.ObjectModel;
using System.Linq;
using System;
using System.ComponentModel;

namespace Deskdrop.WinUI
{
    public sealed partial class QuickAccessWindow : Window
    {
        public event EventHandler DashboardRequested;

        public QuickAccessWindow()
        {
            this.InitializeComponent();
            ExtendsContentIntoTitleBar = true;
            SetTitleBar(AppTitleBar);

            // Resize the window
            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            var windowId = Microsoft.UI.Win32Interop.GetWindowIdFromWindow(hwnd);
            var appWindow = Microsoft.UI.Windowing.AppWindow.GetFromWindowId(windowId);
            appWindow.Resize(new Windows.Graphics.SizeInt32(360, 600));

            TimelineList.ItemsSource = DeskdropStore.Shared.History;
            if (DeviceTargetsList != null) DeviceTargetsList.ItemsSource = DeskdropStore.Shared.Peers;
            DeskdropStore.Shared.PropertyChanged += OnStoreChanged;
        }

        private void OnStoreChanged(object sender, PropertyChangedEventArgs e)
        {
            DispatcherQueue.TryEnqueue(() => {
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

        private void BtnHeaderDiagnostics_Click(object sender, RoutedEventArgs e)
        {
            DashboardRequested?.Invoke(this, EventArgs.Empty);
            Close();
        }

        private void BtnHeaderQuit_Click(object sender, RoutedEventArgs e)
        {
            Application.Current.Exit();
        }

        private void TxtSearch_TextChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
        {
            var query = TxtSearch.Text.ToLowerInvariant();
            if (string.IsNullOrWhiteSpace(query))
            {
                TimelineList.ItemsSource = DeskdropStore.Shared.History;
            }
            else
            {
                TimelineList.ItemsSource = DeskdropStore.Shared.History
                    .Where(h => (h.display_text?.ToLowerInvariant().Contains(query) == true) || 
                                (h.path?.ToLowerInvariant().Contains(query) == true))
                    .ToList();
            }
        }

        private void BtnPinItem_Click(object sender, RoutedEventArgs e)
        {
            // Pin not supported in current model
        }

        private void BtnDeleteItem_Click(object sender, RoutedEventArgs e)
        {
            if (((FrameworkElement)sender).DataContext is HistoryItem item)
            {
                DeskdropStore.Shared.History.Remove(item);
                DeskdropStore.Shared.TriggerHistoryUpdate();
            }
        }

        private void HistoryItem_Click(object sender, RoutedEventArgs e)
        {
            if (((FrameworkElement)sender).DataContext is HistoryItem item)
            {
                // Apply to local clipboard
                if (item.is_text && !string.IsNullOrEmpty(item.display_text))
                {
                    try {
                        var dp = new Windows.ApplicationModel.DataTransfer.DataPackage();
                        dp.SetText(item.display_text);
                        Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(dp);
                    } catch { }
                }
                Close();
            }
        }

        private void DeviceTarget_Click(object sender, RoutedEventArgs e)
        {
            if (((FrameworkElement)sender).DataContext is PeerViewModel peer)
            {
                System.Threading.Tasks.Task.Run(() => 
                {
                    try 
                    {
                        var req = new {
                            cmd = "push_clipboard",
                            target_device = peer.device_id,
                            text = "WinUI Clipboard (Placeholder)"
                        };
                        DaemonClient.Send(req);
                    } 
                    catch { }
                });
                Close();
            }
        }
    }
}


