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

        private void DropGrid_Drop(object sender, Microsoft.UI.Xaml.DragEventArgs e)
        {
            DropGrid.Background = new SolidColorBrush(Windows.UI.Color.FromArgb(128, 0, 0, 0)); // Revert
            // Handle drop logic
        }
    }
}

