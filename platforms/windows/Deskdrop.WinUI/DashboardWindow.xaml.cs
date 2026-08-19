using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Deskdrop.WinUI.Views;

namespace Deskdrop.WinUI
{
    public sealed partial class DashboardWindow : Window
    {
        public static new DashboardWindow? Current { get; private set; }
        public DeskdropStore mgr => DeskdropStore.Shared;
        public string localMachineName => System.Environment.MachineName;
        public System.Windows.Input.ICommand ShowMainWindowCommand => ((App)App.Current).ShowMainWindowCommand;

        public DashboardWindow()
        {
            Current = this;
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
            appWindow.Title = "Deskdrop";
            appWindow.IsShownInSwitchers = true;
            appWindow.Resize(new Windows.Graphics.SizeInt32(1180, 740));



            appWindow.Closing += (s, e) =>
            {
                if (!App.IsShuttingDown)
                {
                    e.Cancel = true;
                    // Minimize to taskbar using robust Win32 call
                    ShowWindow(hwnd, 6 /* SW_MINIMIZE */);
                }
            };

            this.Closed += (s, e) =>
            {
                if (Current == this) Current = null;
            };

            // Default to exact 3-panel Devices/Launchpad view matching macOS
            NavView.Loaded += (s, e) =>
            {
                NavView.SelectedItem = NavDevices;
            };
        }

        [System.Runtime.InteropServices.DllImport("user32.dll")]
        private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

        public static Microsoft.UI.Xaml.Media.Brush GetBrushFromHex(string hex)
        {
            if (string.IsNullOrWhiteSpace(hex)) return new Microsoft.UI.Xaml.Media.SolidColorBrush(Microsoft.UI.Colors.Transparent);
            try
            {
                if (hex.StartsWith("#")) hex = hex.Substring(1);
                if (hex.Length == 6)
                {
                    byte r = byte.Parse(hex.Substring(0, 2), System.Globalization.NumberStyles.HexNumber);
                    byte g = byte.Parse(hex.Substring(2, 2), System.Globalization.NumberStyles.HexNumber);
                    byte b = byte.Parse(hex.Substring(4, 2), System.Globalization.NumberStyles.HexNumber);
                    return new Microsoft.UI.Xaml.Media.SolidColorBrush(Windows.UI.Color.FromArgb(255, r, g, b));
                }
            }
            catch { }
            return new Microsoft.UI.Xaml.Media.SolidColorBrush(Microsoft.UI.Colors.Transparent);
        }

        private void NavView_SelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
        {
            if (args.SelectedItem is NavigationViewItem item)
            {
                switch (item.Tag)
                {
                    case "Devices":
                        ContentFrame.Navigate(typeof(DevicesView));
                        break;
                    case "DevicePeer":
                        ContentFrame.Navigate(typeof(RemoteExplorerView));
                        break;
                    case "Clipboard":
                        ContentFrame.Navigate(typeof(ClipboardView));
                        break;
                    case "Transfers":
                        ContentFrame.Navigate(typeof(TransfersView));
                        break;
                    case "Settings":
                        ContentFrame.Navigate(typeof(SettingsView));
                        break;
                    case "Activity":
                        ContentFrame.Navigate(typeof(ActivityView));
                        break;
                }
            }
        }

        public void NavigateTo(string tag)
        {
            foreach (var item in NavView.MenuItems)
            {
                if (item is NavigationViewItem navItem && navItem.Tag as string == tag)
                {
                    NavView.SelectedItem = navItem;
                    break;
                }
            }
        }



        private void OnTitleBarScanClicked(object sender, RoutedEventArgs e)
        {
            mgr.UpdateStateFromDaemon();
        }

        private async void OnTitleBarSendClicked(object sender, RoutedEventArgs e)
        {
            try
            {
                var picker = new Windows.Storage.Pickers.FileOpenPicker();
                var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
                WinRT.Interop.InitializeWithWindow.Initialize(picker, hwnd);
                picker.FileTypeFilter.Add("*");
                picker.SuggestedStartLocation = Windows.Storage.Pickers.PickerLocationId.Downloads;
                var files = await picker.PickMultipleFilesAsync();
                if (files != null && files.Count > 0)
                {
                    var firstConnected = mgr.ConnectedPeers.FirstOrDefault();
                    foreach (var file in files)
                    {
                        DaemonClient.SendFilePath(firstConnected?.device_id, file.Path, file.Name, file.ContentType);
                    }
                    NavigateTo("Transfers");
                }
            }
            catch (System.Exception ex)
            {
                App.HandleError(ex);
            }
        }

        private void Quit_Click(object sender, RoutedEventArgs e)
        {
            ((App)App.Current).ExitApplicationCommand?.Execute(null);
        }
    }
}
