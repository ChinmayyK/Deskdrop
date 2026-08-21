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

        private Microsoft.UI.Windowing.AppWindow? _appWindow;

        public DashboardWindow()
        {
            var dir = System.IO.Path.Combine(System.Environment.GetFolderPath(System.Environment.SpecialFolder.LocalApplicationData), "Deskdrop");
            System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + System.DateTime.Now.ToString("u") + "] DashboardWindow constructor starting\n");
            
            Current = this;
            this.InitializeComponent();

            System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + System.DateTime.Now.ToString("u") + "] InitializeComponent completed\n");

            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + System.DateTime.Now.ToString("u") + "] DashboardWindow HWND=0x" + hwnd.ToString("X") + "\n");
            var windowId = Microsoft.UI.Win32Interop.GetWindowIdFromWindow(hwnd);
            _appWindow = Microsoft.UI.Windowing.AppWindow.GetFromWindowId(windowId);
            _appWindow.Title = "Deskdrop";
            _appWindow.IsShownInSwitchers = true;

            this.ExtendsContentIntoTitleBar = true;
            this.SetTitleBar(AppTitleBar);

            _appWindow.Resize(new Windows.Graphics.SizeInt32(1180, 740));
            _appWindow.Move(new Windows.Graphics.PointInt32(120, 80));
            _appWindow.Show(true);

            _appWindow.Closing += (s, e) =>
            {
                System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + System.DateTime.Now.ToString("u") + "] appWindow.Closing fired. IsShuttingDown=" + App.IsShuttingDown + "\n");
                if (!App.IsShuttingDown)
                {
                    e.Cancel = true;
                    // Hide to the system tray rather than minimizing to the
                    // taskbar - Deskdrop.Tray (see App.xaml.cs / TrayService)
                    // restores the window from here via FindWindow + SW_RESTORE.
                    _appWindow.Hide();
                }
            };

            this.Closed += (s, e) =>
            {
                System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + System.DateTime.Now.ToString("u") + "] DashboardWindow.Closed fired!\n");
                if (Current == this) Current = null;
            };

            // Set initial page in ContentFrame
            ContentFrame.Navigate(typeof(Views.DevicesView));
            NavView.SelectedItem = NavDevices;
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
            var dir = System.IO.Path.Combine(System.Environment.GetFolderPath(System.Environment.SpecialFolder.LocalApplicationData), "Deskdrop");
            if (args.SelectedItem is NavigationViewItem item)
            {
                var tag = item.Tag as string;
                System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + System.DateTime.Now.ToString("u") + "] NavView_SelectionChanged: " + tag + "\n");
                try
                {
                    Type pageType = tag switch
                    {
                        "Devices" => typeof(DevicesView),
                        "DevicePeer" => typeof(RemoteExplorerView),
                        "Clipboard" => typeof(ClipboardView),
                        "Transfers" => typeof(TransfersView),
                        "Settings" => typeof(SettingsView),
                        "Activity" => typeof(ActivityView),
                        _ => typeof(DevicesView)
                    };

                    if (ContentFrame.CurrentSourcePageType != pageType)
                    {
                        ContentFrame.Navigate(pageType);
                    }
                }
                catch (System.Exception ex)
                {
                    System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + System.DateTime.Now.ToString("u") + "] Navigation Error for " + tag + ": " + ex.ToString() + "\n");
                }
            }
        }

        public void NavigateTo(string tag)
        {
            var item = NavView.MenuItems.OfType<NavigationViewItem>().FirstOrDefault(i => (string)i.Tag == tag)
                       ?? NavView.FooterMenuItems.OfType<NavigationViewItem>().FirstOrDefault(i => (string)i.Tag == tag);
            if (item != null && NavView.SelectedItem != item)
            {
                NavView.SelectedItem = item;
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
