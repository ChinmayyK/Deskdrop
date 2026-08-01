using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Xaml.Media;

namespace Deskdrop.WinUI
{
    public sealed partial class OnboardingWindow : Window
    {
        public OnboardingWindow()
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

            ExtendsContentIntoTitleBar = true;
            SetTitleBar(AppTitleBar);

            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            var windowId = Microsoft.UI.Win32Interop.GetWindowIdFromWindow(hwnd);
            var appWindow = Microsoft.UI.Windowing.AppWindow.GetFromWindowId(windowId);
            
            appWindow.Resize(new Windows.Graphics.SizeInt32(600, 500));
        }

        private void BtnShowQRCode_Click(object sender, RoutedEventArgs e)
        {
            try
            {
                var qr = new QRPairingWindow();
                qr.Activate();
            }
            catch (Exception ex) { System.Diagnostics.Debug.WriteLine($"Swallowed Exception: {ex.Message}\n{ex.StackTrace}"); }
        }

        private void BtnFooterLeft_Click(object sender, RoutedEventArgs e)
        {
            this.Close();
        }

        private void BtnFooterRight_Click(object sender, RoutedEventArgs e)
        {
            this.Close();
        }
    }
}

