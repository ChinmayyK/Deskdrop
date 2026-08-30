using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
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
            TraceLog.Write("DashboardWindow constructor starting");

            Current = this;
            this.InitializeComponent();

            TraceLog.Write("InitializeComponent completed");

            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            TraceLog.Write("DashboardWindow HWND=0x" + hwnd.ToString("X"));
            var windowId = Microsoft.UI.Win32Interop.GetWindowIdFromWindow(hwnd);
            _appWindow = Microsoft.UI.Windowing.AppWindow.GetFromWindowId(windowId);
            Deskdrop.WinUI.Services.WindowIconHelper.Apply(_appWindow);
            _appWindow.Title = "Deskdrop";
            // Deskdrop lives in the system tray, not the taskbar/alt-tab -
            // it's opened via the tray icon (see TrayService/App.xaml.cs).
            // IsShownInSwitchers alone doesn't reliably drop the taskbar
            // button for an unowned top-level window, so also apply the
            // classic WS_EX_TOOLWINDOW/~WS_EX_APPWINDOW combination directly.
            _appWindow.IsShownInSwitchers = false;
            HideFromTaskbar(hwnd);

            this.ExtendsContentIntoTitleBar = true;
            this.SetTitleBar(AppTitleBar);
            Deskdrop.WinUI.Services.ThemeService.Register(this);

            _appWindow.Resize(new Windows.Graphics.SizeInt32(1180, 740));
            _appWindow.Move(new Windows.Graphics.PointInt32(120, 80));
            _appWindow.Show(true);

            _appWindow.Closing += (s, e) =>
            {
                TraceLog.Write("appWindow.Closing fired. IsShuttingDown=" + App.IsShuttingDown);
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
                TraceLog.Write("DashboardWindow.Closed fired!");
                if (Current == this) Current = null;
            };

            // Set initial page in ContentFrame
            ContentFrame.Navigate(typeof(Views.DevicesView));
            NavView.SelectedItem = NavDevices;
        }

        [System.Runtime.InteropServices.DllImport("user32.dll")]
        private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

        private const int GWL_EXSTYLE = -20;
        private const long WS_EX_TOOLWINDOW = 0x00000080;
        private const long WS_EX_APPWINDOW = 0x00040000;

        [System.Runtime.InteropServices.DllImport("user32.dll", EntryPoint = "GetWindowLongPtr", SetLastError = true)]
        private static extern IntPtr GetWindowLongPtr64(IntPtr hWnd, int nIndex);
        [System.Runtime.InteropServices.DllImport("user32.dll", EntryPoint = "GetWindowLong", SetLastError = true)]
        private static extern int GetWindowLong32(IntPtr hWnd, int nIndex);
        [System.Runtime.InteropServices.DllImport("user32.dll", EntryPoint = "SetWindowLongPtr", SetLastError = true)]
        private static extern IntPtr SetWindowLongPtr64(IntPtr hWnd, int nIndex, IntPtr dwNewLong);
        [System.Runtime.InteropServices.DllImport("user32.dll", EntryPoint = "SetWindowLong", SetLastError = true)]
        private static extern int SetWindowLong32(IntPtr hWnd, int nIndex, int dwNewLong);

        private static long GetWindowLongSafe(IntPtr hWnd, int nIndex) =>
            IntPtr.Size == 8 ? GetWindowLongPtr64(hWnd, nIndex).ToInt64() : GetWindowLong32(hWnd, nIndex);

        private static void SetWindowLongSafe(IntPtr hWnd, int nIndex, long newLong)
        {
            if (IntPtr.Size == 8) SetWindowLongPtr64(hWnd, nIndex, new IntPtr(newLong));
            else SetWindowLong32(hWnd, nIndex, (int)newLong);
        }

        private static void HideFromTaskbar(IntPtr hwnd)
        {
            try
            {
                var exStyle = GetWindowLongSafe(hwnd, GWL_EXSTYLE);
                exStyle |= WS_EX_TOOLWINDOW;
                exStyle &= ~WS_EX_APPWINDOW;
                SetWindowLongSafe(hwnd, GWL_EXSTYLE, exStyle);
            }
            catch (Exception ex) { App.HandleError(ex); }
        }

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
                var tag = item.Tag as string;
                TraceLog.Write("NavView_SelectionChanged: " + tag);
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

                    SetPageTitle(tag);

                    if (ContentFrame.CurrentSourcePageType != pageType)
                    {
                        ContentFrame.Navigate(pageType);
                    }
                }
                catch (System.Exception ex)
                {
                    TraceLog.Write("Navigation Error for " + tag + ": " + ex.ToString());
                    TraceLog.Flush();
                }
            }
        }

        // The page heading lives in the title bar rather than being repeated
        // at the top of every page. That removes a whole band of vertical
        // space from each screen and keeps the heading in one predictable
        // place, which is how Windows' own utilities behave.
        private void SetPageTitle(string? tag)
        {
            if (PageTitleText == null) return;

            PageTitleText.Text = tag switch
            {
                "Devices" => "Ecosystem",
                "DevicePeer" => "Remote files",
                "Clipboard" => "Clipboard",
                "Transfers" => "Transfers",
                "Activity" => "Activity",
                "Settings" => "Settings",
                _ => "Ecosystem",
            };
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
                    var target = await Deskdrop.WinUI.Services.DevicePicker.PickAsync((this.Content as FrameworkElement)?.XamlRoot, mgr.ConnectedPeers);
                    if (target == null && mgr.ConnectedPeers.Count > 0) return; // user cancelled the picker
                    foreach (var file in files)
                    {
                        // SendFilePath's signature is (path, name, mime, targetDevice, ...) -
                        // this used to pass them in FFI order (deviceId first), which
                        // silently scrambled every argument: the device id landed in the
                        // path field, the real path in name, the name in mime, and the
                        // mime type in targetDevice. The daemon then had no real path to
                        // read and no real device to send to, so nothing ever arrived.
                        DaemonClient.SendFilePath(file.Path, file.Name, file.ContentType, target?.device_id);
                    }
                    NavigateTo("Transfers");
                }
            }
            catch (System.Exception ex)
            {
                App.HandleError(ex);
            }
        }

        private void OnRescanClicked(object sender, RoutedEventArgs e)
        {
            DaemonClient.RescanPeers();
            mgr.UpdateStateFromDaemon();
        }

        private void OnOpenSettingsClicked(object sender, RoutedEventArgs e)
        {
            NavigateTo("Settings");
        }

        private void OnOpenDownloadsClicked(object sender, RoutedEventArgs e)
        {
            try
            {
                var downloadsPath = System.IO.Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), "Downloads");
                System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo
                {
                    FileName = downloadsPath,
                    UseShellExecute = true
                });
            }
            catch (System.Exception ex)
            {
                App.HandleError(ex);
            }
        }

        // Pairing is a focused, in-app flow now (see PairDeviceDialog) rather
        // than a separate top-level window competing for the taskbar.
        private async void OnPairDeviceClicked(object sender, RoutedEventArgs e)
        {
            try
            {
                var root = this.Content as FrameworkElement;
                if (root?.XamlRoot == null) return;
                await new PairDeviceDialog { XamlRoot = root.XamlRoot }.ShowAsync();
            }
            catch (System.Exception ex)
            {
                App.HandleError(ex);
            }
        }

        // ---- Drag and drop onto the window ---------------------------
        //
        // Files dropped anywhere on the content area are sent, with the same
        // target resolution as the Send file button: straight through when
        // one device is connected, prompt when the choice is ambiguous.

        private void OnContentDragOver(object sender, DragEventArgs e)
        {
            if (!e.DataView.Contains(Windows.ApplicationModel.DataTransfer.StandardDataFormats.StorageItems))
            {
                e.AcceptedOperation = Windows.ApplicationModel.DataTransfer.DataPackageOperation.None;
                return;
            }

            e.AcceptedOperation = Windows.ApplicationModel.DataTransfer.DataPackageOperation.Copy;

            // Our own overlay says what will happen, so suppress the shell's
            // "+ Copy" badge rather than showing two competing captions.
            if (e.DragUIOverride != null)
            {
                e.DragUIOverride.IsCaptionVisible = false;
                e.DragUIOverride.IsGlyphVisible = false;
            }

            // Say up front where it's going, so the drop isn't a guess.
            DropOverlayDetail.Text = mgr.ConnectedCount switch
            {
                0 => "No devices are connected - pair one first",
                1 => $"Release to send to {mgr.ConnectedPeers.FirstOrDefault()?.DisplayName ?? "your device"}",
                _ => "Release to choose a device to send to",
            };
            DropOverlay.Visibility = Visibility.Visible;
        }

        private void OnContentDragLeave(object sender, DragEventArgs e)
        {
            DropOverlay.Visibility = Visibility.Collapsed;
        }

        private async void OnContentDrop(object sender, DragEventArgs e)
        {
            DropOverlay.Visibility = Visibility.Collapsed;

            try
            {
                if (!e.DataView.Contains(Windows.ApplicationModel.DataTransfer.StandardDataFormats.StorageItems)) return;

                var items = await e.DataView.GetStorageItemsAsync();
                if (items == null || items.Count == 0) return;

                var files = items.OfType<Windows.Storage.StorageFile>().ToList();
                if (files.Count == 0) return;

                var target = await Deskdrop.WinUI.Services.DevicePicker.PickAsync(
                    (this.Content as FrameworkElement)?.XamlRoot, mgr.ConnectedPeers);
                if (target == null && mgr.ConnectedPeers.Count > 0) return; // user cancelled

                foreach (var file in files)
                {
                    // Argument order is (path, name, mime, targetDevice, ...); see the
                    // matching fix note above in OnTitleBarSendClicked.
                    DaemonClient.SendFilePath(file.Path, file.Name, file.ContentType, target?.device_id);
                }
                NavigateTo("Transfers");
            }
            catch (System.Exception ex)
            {
                App.HandleError(ex);
            }
        }

        private void OnPairAcceleratorInvoked(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
        {
            args.Handled = true;
            OnPairDeviceClicked(sender, new RoutedEventArgs());
        }

        private void Quit_Click(object sender, RoutedEventArgs e)
        {
            ((App)App.Current).ExitApplicationCommand?.Execute(null);
        }
    }
}
