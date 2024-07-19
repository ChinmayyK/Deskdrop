using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Windowing;
using Microsoft.UI.Composition.SystemBackdrops;
using WinRT.Interop;
using System.Collections.ObjectModel;

namespace Deskdrop.WinUI
{
    public sealed partial class QuickAccessWindow : Window
    {
        public QuickAccessWindow()
        {
            this.InitializeComponent();

            this.SystemBackdrop = new DesktopAcrylicBackdrop();
            ExtendsContentIntoTitleBar = true;
            SetTitleBar(AppTitleBar);

            // Resize the window
            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            var windowId = Microsoft.UI.Win32Interop.GetWindowIdFromWindow(hwnd);
            var appWindow = Microsoft.UI.Windowing.AppWindow.GetFromWindowId(windowId);
            appWindow.Resize(new Windows.Graphics.SizeInt32(360, 600));
        }

        private void BtnHeaderDiagnostics_Click(object sender, RoutedEventArgs e)
        {
            // Open diagnostics
        }

        private void BtnHeaderQuit_Click(object sender, RoutedEventArgs e)
        {
            Application.Current.Exit();
        }

        private void TxtSearch_TextChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
        {
            // Search logic
        }

        private void BtnPinItem_Click(object sender, RoutedEventArgs e)
        {
            // Pin logic
        }

        private void BtnDeleteItem_Click(object sender, RoutedEventArgs e)
        {
            // Delete logic
        }
    }
}
