using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Deskdrop.WinUI
{
    public sealed partial class DropZoneWindow : Window
    {
        public DropZoneWindow()
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

            // Make the window transparent and click-through where appropriate
            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            var windowId = Microsoft.UI.Win32Interop.GetWindowIdFromWindow(hwnd);
            var appWindow = Microsoft.UI.Windowing.AppWindow.GetFromWindowId(windowId);
            var presenter = appWindow.Presenter as Microsoft.UI.Windowing.OverlappedPresenter;
            if (presenter != null)
            {
                presenter.IsAlwaysOnTop = true;
                presenter.IsResizable = false;
                presenter.IsMaximizable = false;
                presenter.SetBorderAndTitleBar(false, false);
            }
        }

        private void CloseButton_Click(object sender, RoutedEventArgs e)
        {
            this.Close();
        }

        private void DropGrid_DragEnter(object sender, Microsoft.UI.Xaml.DragEventArgs e)
        {
            e.AcceptedOperation = Windows.ApplicationModel.DataTransfer.DataPackageOperation.Copy;
            DropGrid.Background = new SolidColorBrush(Windows.UI.Color.FromArgb(200, 10, 132, 255)); // Apple Blue #0A84FF
        }

        private void DropGrid_DragLeave(object sender, Microsoft.UI.Xaml.DragEventArgs e)
        {
            DropGrid.Background = new SolidColorBrush(Windows.UI.Color.FromArgb(128, 0, 0, 0)); // Revert
        }

        private async void DropGrid_Drop(object sender, Microsoft.UI.Xaml.DragEventArgs e)
        {
            DropGrid.Background = new SolidColorBrush(Windows.UI.Color.FromArgb(128, 0, 0, 0)); // Revert
            try
            {
                var peers = System.Linq.Enumerable.ToList(DeskdropStore.Shared.Peers);
                var target = peers.Count > 0 ? peers[0].device_id : "";
                if (string.IsNullOrEmpty(target)) return;

                if (e.DataView.Contains(Windows.ApplicationModel.DataTransfer.StandardDataFormats.StorageItems))
                {
                    var items = await e.DataView.GetStorageItemsAsync();
                    if (items.Count > 0 && items[0] is Windows.Storage.StorageFile file)
                    {
                        System.Threading.Tasks.Task.Run(() => DaemonClient.PushFile(target, file.Path));
                    }
                }
                else if (e.DataView.Contains(Windows.ApplicationModel.DataTransfer.StandardDataFormats.Text))
                {
                    var text = await e.DataView.GetTextAsync();
                    System.Threading.Tasks.Task.Run(() => DaemonClient.PushTextTo(text, target));
                }
            }
            catch (Exception ex) { System.Diagnostics.Debug.WriteLine($"Swallowed Exception: {ex.Message}\n{ex.StackTrace}"); }
        }
    }
}

