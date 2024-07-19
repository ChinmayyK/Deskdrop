using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Composition.SystemBackdrops;

namespace Deskdrop.WinUI
{
    public sealed partial class IncomingFileBannerWindow : Window
    {
        public IncomingFileBannerWindow()
        {
            this.InitializeComponent();

            this.SystemBackdrop = new DesktopAcrylicBackdrop();
            ExtendsContentIntoTitleBar = true;
            SetTitleBar(AppTitleBar);

            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            var windowId = Microsoft.UI.Win32Interop.GetWindowIdFromWindow(hwnd);
            var appWindow = Microsoft.UI.Windowing.AppWindow.GetFromWindowId(windowId);
            
            // Set to overlap without borders and un-resizable
            var presenter = appWindow.Presenter as Microsoft.UI.Windowing.OverlappedPresenter;
            if (presenter != null)
            {
                presenter.IsAlwaysOnTop = true;
                presenter.IsResizable = false;
                presenter.IsMaximizable = false;
                presenter.SetBorderAndTitleBar(false, false);
            }
            appWindow.Resize(new Windows.Graphics.SizeInt32(360, 100));
        }

        private void BtnAccept_Click(object sender, RoutedEventArgs e)
        {
            this.Close();
        }
    }
}
