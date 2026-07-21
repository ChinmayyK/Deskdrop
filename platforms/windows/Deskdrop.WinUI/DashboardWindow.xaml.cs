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
            appWindow.Resize(new Windows.Graphics.SizeInt32(960, 720));

            // Default to Activity View
            NavView.Loaded += (s, e) =>
            {
                NavView.SelectedItem = NavActivity;
            };
            ContentFrame.Navigate(typeof(ActivityView));
        }

        private void NavView_SelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
        {
            if (args.IsSettingsSelected)
            {
                // ContentFrame.Navigate(typeof(SettingsView)); // Settings view to be implemented later if needed
            }
            else if (args.SelectedItem is NavigationViewItem item)
            {
                switch (item.Tag)
                {
                    case "Activity":
                        ContentFrame.Navigate(typeof(ActivityView));
                        break;
                    case "Devices":
                        ContentFrame.Navigate(typeof(DevicesView));
                        break;
                    case "Transfers":
                        ContentFrame.Navigate(typeof(TransfersView));
                        break;
                }
            }
        }
    }
}
