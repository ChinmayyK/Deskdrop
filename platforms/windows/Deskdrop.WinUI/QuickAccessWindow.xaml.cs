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

            if (Microsoft.UI.Composition.SystemBackdrops.MicaController.IsSupported())
            {
                this.SystemBackdrop = new Microsoft.UI.Xaml.Media.MicaBackdrop();
            }
            else if (Microsoft.UI.Composition.SystemBackdrops.DesktopAcrylicController.IsSupported())
            {
                this.SystemBackdrop = new Microsoft.UI.Xaml.Media.DesktopAcrylicBackdrop();
            }

            // Resize the window
            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            var windowId = Microsoft.UI.Win32Interop.GetWindowIdFromWindow(hwnd);
            var appWindow = Microsoft.UI.Windowing.AppWindow.GetFromWindowId(windowId);
            appWindow.Resize(new Windows.Graphics.SizeInt32(360, 600));

            TimelineList.ItemsSource = DeskdropStore.Shared.History;
            if (DeviceTargetsList != null) DeviceTargetsList.ItemsSource = DeskdropStore.Shared.Peers;
            DeskdropStore.Shared.PropertyChanged += OnStoreChanged;
            this.Closed += (s, e) => {
                DeskdropStore.Shared.PropertyChanged -= OnStoreChanged;
            };
        }

        private void OnStoreChanged(object sender, PropertyChangedEventArgs e)
        {
            DispatcherQueue?.TryEnqueue(() => {
                try
                {
                    if (e.PropertyName == nameof(DeskdropStore.History))
                    {
                        if (string.IsNullOrWhiteSpace(TxtSearch?.Text))
                            TimelineList.ItemsSource = DeskdropStore.Shared.History;
                    }
                    else if (e.PropertyName == nameof(DeskdropStore.Peers) && DeviceTargetsList != null)
                    {
                        DeviceTargetsList.ItemsSource = DeskdropStore.Shared.Peers;
                    }
                }
                catch (Exception ex) { System.Diagnostics.Debug.WriteLine($"Swallowed Exception: {ex.Message}\n{ex.StackTrace}"); }
            });
        }

        private void BtnHeaderDiagnostics_Click(object sender, RoutedEventArgs e)
        {
            DashboardRequested?.Invoke(this, EventArgs.Empty);
            Close();
        }

        private void BtnHeaderQuit_Click(object sender, RoutedEventArgs e)
        {
            if (Application.Current is App app)
            {
                app.ExitApplicationCommand.Execute(null);
            }
            else
            {
                Application.Current.Exit();
                Environment.Exit(0);
            }
        }

        private void TxtSearch_TextChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
        {
            try
            {
                var query = TxtSearch?.Text?.ToLowerInvariant() ?? "";
                var snapshot = DeskdropStore.Shared.History.ToList();
                if (string.IsNullOrWhiteSpace(query))
                {
                    TimelineList.ItemsSource = snapshot;
                }
                else
                {
                    TimelineList.ItemsSource = snapshot
                        .Where(h => (h.display_text?.ToLowerInvariant().Contains(query) == true) || 
                                    (h.path?.ToLowerInvariant().Contains(query) == true))
                        .ToList();
                }
            }
            catch (Exception ex) { System.Diagnostics.Debug.WriteLine($"Swallowed Exception: {ex.Message}\n{ex.StackTrace}"); }
        }

        private void BtnPinItem_Click(object sender, RoutedEventArgs e)
        {
            try
            {
                if (((FrameworkElement)sender).DataContext is HistoryItem item)
                {
                    item.IsPinned = !item.IsPinned;
                    var snapshot = DeskdropStore.Shared.History.ToList();
                    TimelineList.ItemsSource = snapshot
                        .OrderByDescending(h => h.IsPinned)
                        .ThenByDescending(h => h.Time)
                        .ToList();
                }
            }
            catch (Exception ex) { System.Diagnostics.Debug.WriteLine($"Swallowed Exception: {ex.Message}\n{ex.StackTrace}"); }
        }

        private void BtnDeleteItem_Click(object sender, RoutedEventArgs e)
        {
            try
            {
                if (((FrameworkElement)sender).DataContext is HistoryItem item)
                {
                    DeskdropStore.Shared.History.Remove(item);
                    App.Clipboard?.History.Remove(item);
                    DeskdropStore.Shared.TriggerHistoryUpdate();
                }
            }
            catch (Exception ex) { System.Diagnostics.Debug.WriteLine($"Swallowed Exception: {ex.Message}\n{ex.StackTrace}"); }
        }

        private void HistoryItem_Click(object sender, RoutedEventArgs e)
        {
            if (((FrameworkElement)sender).DataContext is HistoryItem item)
            {
                if (item.is_text && !string.IsNullOrEmpty(item.FullText))
                {
                    try {
                        var dp = new Windows.ApplicationModel.DataTransfer.DataPackage();
                        dp.SetText(item.FullText);
                        Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(dp);
                    } catch (Exception ex) { System.Diagnostics.Debug.WriteLine($"Swallowed Exception: {ex.Message}\n{ex.StackTrace}"); }
                }
                else if (!string.IsNullOrEmpty(item.path) && System.IO.File.Exists(item.path))
                {
                    try {
                        System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(item.path) { UseShellExecute = true });
                    } catch (Exception ex) { System.Diagnostics.Debug.WriteLine($"Swallowed Exception: {ex.Message}\n{ex.StackTrace}"); }
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
                        DaemonClient.PushClipboard(peer.device_id);
                    } 
                    catch (Exception ex) { System.Diagnostics.Debug.WriteLine($"Swallowed Exception: {ex.Message}\n{ex.StackTrace}"); }
                });
                Close();
            }
        }
    }
}


