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

            this.SystemBackdrop = new DesktopAcrylicBackdrop();
            ExtendsContentIntoTitleBar = true;
            SetTitleBar(AppTitleBar);

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
    }
}
