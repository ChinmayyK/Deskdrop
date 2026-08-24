using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;

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
            Deskdrop.WinUI.Services.ThemeService.Register(this);

            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            var windowId = Microsoft.UI.Win32Interop.GetWindowIdFromWindow(hwnd);
            var appWindow = Microsoft.UI.Windowing.AppWindow.GetFromWindowId(windowId);
            Deskdrop.WinUI.Services.WindowIconHelper.Apply(appWindow);
            
            appWindow.Resize(new Windows.Graphics.SizeInt32(600, 500));

            CardRoot.Loaded += (s, e) => PlayEntranceAnimation();
        }

        private void PlayEntranceAnimation()
        {
            var storyboard = new Storyboard();
            var easing = new CubicEase { EasingMode = EasingMode.EaseOut };

            var fade = new DoubleAnimation { To = 1.0, Duration = TimeSpan.FromMilliseconds(280), EasingFunction = easing };
            Storyboard.SetTarget(fade, CardRoot);
            Storyboard.SetTargetProperty(fade, "Opacity");

            var rise = new DoubleAnimation { To = 0.0, Duration = TimeSpan.FromMilliseconds(280), EasingFunction = easing };
            Storyboard.SetTarget(rise, CardTranslate);
            Storyboard.SetTargetProperty(rise, "Y");

            storyboard.Children.Add(fade);
            storyboard.Children.Add(rise);
            storyboard.Begin();
        }

        private void BtnShowQRCode_Click(object sender, RoutedEventArgs e)
        {
            try
            {
                var qr = new QRPairingWindow();
                qr.Activate();
            }
            catch (Exception ex) { App.HandleError(ex); }
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

