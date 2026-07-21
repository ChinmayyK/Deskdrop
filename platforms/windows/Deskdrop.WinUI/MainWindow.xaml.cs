using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using System.Collections.ObjectModel;
using WinRT.Interop;
using Microsoft.UI.Windowing;
using Microsoft.UI.Composition.SystemBackdrops;

namespace Deskdrop.WinUI
{
    public sealed partial class MainWindow : Window
    {
        public ObservableCollection<string> RecentTransfers { get; set; } = new();

        public MainWindow()
        {
            this.InitializeComponent();

            if (Microsoft.UI.Composition.SystemBackdrops.MicaController.IsSupported())
            {
                this.SystemBackdrop = new Microsoft.UI.Xaml.Media.MicaBackdrop();
            }
            else if (Microsoft.UI.Composition.SystemBackdrops.DesktopAcrylicController.IsSupported())
            {
                this.SystemBackdrop = new Microsoft.UI.Xaml.Media.DesktopAcrylicBackdrop();
            }

            // Optional: Hide default title bar to mimic macOS frameless look
            // ExtendsContentIntoTitleBar = true;
            // SetTitleBar(AppTitleBar);

            // Resize the window
            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            var windowId = Microsoft.UI.Win32Interop.GetWindowIdFromWindow(hwnd);
            var appWindow = Microsoft.UI.Windowing.AppWindow.GetFromWindowId(windowId);
            appWindow.Resize(new Windows.Graphics.SizeInt32(380, 480));

            // Populate some dummy data for now
            RecentTransfers.Add("screenshot.png - Sent to iPhone");
            RecentTransfers.Add("document.pdf - Received from Mac");
        }

        private void PairDevice_Click(object sender, RoutedEventArgs e)
        {
            var qrWindow = new QRPairingWindow();
            qrWindow.Activate();
        }

        private void BtnOpenDashboard_Click(object sender, RoutedEventArgs e)
        {
            var dashboard = new DashboardWindow();
            dashboard.Activate();
        }
    }
}

