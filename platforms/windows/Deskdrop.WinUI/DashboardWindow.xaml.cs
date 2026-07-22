using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Deskdrop.WinUI.Views;

namespace Deskdrop.WinUI
{
    public sealed partial class DashboardWindow : Window
    {
        public DashboardWindow()
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
            appWindow.Resize(new Windows.Graphics.SizeInt32(1180, 740));

            // Default to exact 3-panel Devices/Launchpad view matching macOS
            NavView.Loaded += (s, e) =>
            {
                NavView.SelectedItem = NavDevices;
            };
            ContentFrame.Navigate(typeof(DevicesView));
        }

        private void NavView_SelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
        {
            if (args.SelectedItem is NavigationViewItem item)
            {
                switch (item.Tag)
                {
                    case "Devices":
                    case "DevicePeer":
                        ContentFrame.Navigate(typeof(DevicesView));
                        break;
                    case "Activity":
                        ContentFrame.Navigate(typeof(ActivityView));
                        break;
                    case "Transfers":
                        ContentFrame.Navigate(typeof(TransfersView));
                        break;
                }
            }
        }
    }
}
