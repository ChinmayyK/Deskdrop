using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Windowing;
using Microsoft.UI.Composition.SystemBackdrops;
using System.Collections.ObjectModel;

namespace Deskdrop.WinUI
{
    public sealed partial class RemoteExplorerWindow : Window
    {
        public RemoteExplorerWindow()
        {
            this.InitializeComponent();
            ExtendsContentIntoTitleBar = true;
            SetTitleBar(AppTitleBar);

            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            var windowId = Microsoft.UI.Win32Interop.GetWindowIdFromWindow(hwnd);
            var appWindow = Microsoft.UI.Windowing.AppWindow.GetFromWindowId(windowId);
            appWindow.Resize(new Windows.Graphics.SizeInt32(600, 500));
        }

        public RemoteExplorerWindow(object context) : this()
        {
            // Set DataContext based on context
        }

        public void Show()
        {
            this.Activate();
        }

        private void BtnBack_Click(object sender, RoutedEventArgs e)
        {
            // Back logic
        }

        private void BtnSelectMode_CheckedChanged(object sender, RoutedEventArgs e)
        {
            if (BatchActionBar != null)
            {
                BatchActionBar.Visibility = BtnSelectMode.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
            }
        }

        private void FileListView_MouseDoubleClick(object sender, Microsoft.UI.Xaml.Input.DoubleTappedRoutedEventArgs e)
        {
            // Open file logic
        }

        private void BtnDownloadBatch_Click(object sender, RoutedEventArgs e)
        {
            // Download logic
        }

        private void BtnDeleteBatch_Click(object sender, RoutedEventArgs e)
        {
            // Delete logic
        }
    }
}


