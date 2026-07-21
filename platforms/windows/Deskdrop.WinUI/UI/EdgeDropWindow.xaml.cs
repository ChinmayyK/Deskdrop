using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Xaml.Media;
using System;

namespace Deskdrop.WinUI.UI
{
    public sealed partial class EdgeDropWindow : Window
    {
        public EdgeDropWindow()
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
            appWindow.Resize(new Windows.Graphics.SizeInt32(200, 300));
        }

        private void Grid_DragEnter(object sender, Microsoft.UI.Xaml.DragEventArgs e)
        {
            e.AcceptedOperation = Windows.ApplicationModel.DataTransfer.DataPackageOperation.Copy;
            RestingSliver.Visibility = Visibility.Collapsed;
            ExpandedCard.Visibility = Visibility.Visible;
            ExpandedCard.Opacity = 1.0;
        }

        private void Grid_DragLeave(object sender, Microsoft.UI.Xaml.DragEventArgs e)
        {
            RestingSliver.Visibility = Visibility.Visible;
            ExpandedCard.Visibility = Visibility.Collapsed;
            ExpandedCard.Opacity = 0;
        }

        private async void Grid_Drop(object sender, Microsoft.UI.Xaml.DragEventArgs e)
        {
            RestingSliver.Visibility = Visibility.Visible;
            ExpandedCard.Visibility = Visibility.Collapsed;
            ExpandedCard.Opacity = 0;
            try
            {
                if (e.DataView.Contains(Windows.ApplicationModel.DataTransfer.StandardDataFormats.StorageItems))
                {
                    var items = await e.DataView.GetStorageItemsAsync();
                    if (items.Count > 0 && items[0] is Windows.Storage.StorageFile file)
                    {
                        var target = DeskdropStore.Shared.Peers.Count > 0 ? DeskdropStore.Shared.Peers[0].device_id : "";
                        if (!string.IsNullOrEmpty(target))
                        {
                            System.Threading.Tasks.Task.Run(() => DaemonClient.PushFile(target, file.Path));
                        }
                    }
                }
                else if (e.DataView.Contains(Windows.ApplicationModel.DataTransfer.StandardDataFormats.Text))
                {
                    var text = await e.DataView.GetTextAsync();
                    var target = DeskdropStore.Shared.Peers.Count > 0 ? DeskdropStore.Shared.Peers[0].device_id : "";
                    if (!string.IsNullOrEmpty(target))
                    {
                        System.Threading.Tasks.Task.Run(() => DaemonClient.PushTextTo(text, target));
                    }
                }
            }
            catch { }
        }
    }
}


