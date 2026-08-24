using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using System;
using System.Linq;
using Microsoft.UI.Xaml.Media.Media3D;

namespace Deskdrop.WinUI.Views
{
    public sealed partial class DevicesView : Page
    {
        public DeskdropStore mgr => DeskdropStore.Shared;

        public DevicesView()
        {
            var dir = System.IO.Path.Combine(System.Environment.GetFolderPath(System.Environment.SpecialFolder.LocalApplicationData), "Deskdrop");
            System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + System.DateTime.Now.ToString("u") + "] DevicesView constructor starting\n");
            this.InitializeComponent();
            System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + System.DateTime.Now.ToString("u") + "] DevicesView InitializeComponent done\n");
        }

        private void OnPeerCardTapped(object sender, TappedRoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is PeerViewModel peer)
            {
                mgr.SelectedPeer = peer;
            }
        }

        private void OnPairingAcceptClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is PeerViewModel peer)
            {
                mgr.RespondToPairing(peer.device_id, true);
            }
        }

        private void OnPairingRejectClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is PeerViewModel peer)
            {
                mgr.RespondToPairing(peer.device_id, false);
            }
        }

        private void OnPairDeviceClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is PeerViewModel peer)
            {
                mgr.ConnectAndPair(peer.device_id);
            }
        }

        // Resolve the peer from the row that was clicked, not from
        // SelectedPeer: the per-row disconnect button used to act on whatever
        // card happened to be selected, so with two devices connected it
        // could disconnect the wrong one.
        private void OnDisconnectClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is PeerViewModel peer)
            {
                mgr.DisconnectPeer(peer.device_id);
            }
        }

        private void OnForgetDeviceMenuClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is PeerViewModel peer)
            {
                mgr.ForgetPeer(peer.device_id);
            }
        }

        private async void OnRenameDeviceClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is not PeerViewModel peer) return;

            var currentName = peer.DisplayName;
            var input = new TextBox { Text = currentName, SelectionStart = 0, SelectionLength = currentName.Length };
            var dialog = new ContentDialog
            {
                Title = "Rename Device",
                Content = input,
                PrimaryButtonText = "Rename",
                CloseButtonText = "Cancel",
                DefaultButton = ContentDialogButton.Primary,
                XamlRoot = this.XamlRoot,
            };

            var result = await dialog.ShowAsync();
            if (result != ContentDialogResult.Primary) return;

            var newName = input.Text?.Trim();
            if (string.IsNullOrEmpty(newName) || newName == currentName) return;

            try
            {
                DaemonClient.RenameTrustedDevice(peer.device_id, newName);
                peer.friendly_name = newName;
            }
            catch (Exception ex) { App.HandleError(ex); }
        }

        private void OnPauseSyncClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is PeerViewModel peer)
            {
                DaemonClient.PauseSyncPeer(peer.device_id);
            }
        }

        private void OnResumeSyncClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is PeerViewModel peer)
            {
                DaemonClient.ResumeSyncPeer(peer.device_id);
            }
        }

        private void OnDisconnectAllClicked(object sender, RoutedEventArgs e)
        {
            DaemonClient.DisconnectAllPeers();
        }

        // "Scan" should actually probe the network, not just re-read cached
        // daemon state - the old handler only did the latter, which made the
        // button look broken when nothing new appeared.
        private void OnScanNearbyClicked(object sender, RoutedEventArgs e)
        {
            DaemonClient.RescanPeers();
            mgr.UpdateStateFromDaemon();
        }

        // Pairing is now an in-app sheet rather than a second top-level
        // window, so the QR is the focus of the screen while it's open and
        // the flow closes when the task is done.
        private async void OnShowQRCodeClicked(object sender, RoutedEventArgs e)
        {
            try
            {
                await new PairDeviceDialog { XamlRoot = this.XamlRoot }.ShowAsync();
            }
            catch (Exception ex)
            {
                App.HandleError(ex);
            }
        }

        private void OnOpenActivityClicked(object sender, RoutedEventArgs e)
        {
            DashboardWindow.Current?.NavigateTo("Activity");
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
            catch (Exception ex)
            {
                App.HandleError(ex);
            }
        }

        // Files first, then target: picking what to send before choosing
        // where it goes matches how people think about the task, and skips
        // the device prompt entirely when there's only one candidate.
        private async void OnQuickSendClicked(object sender, RoutedEventArgs e)
        {
            try
            {
                var picker = new Windows.Storage.Pickers.FileOpenPicker();
                var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(App.MainWindow);
                WinRT.Interop.InitializeWithWindow.Initialize(picker, hwnd);
                picker.FileTypeFilter.Add("*");
                picker.SuggestedStartLocation = Windows.Storage.Pickers.PickerLocationId.Downloads;

                var files = await picker.PickMultipleFilesAsync();
                if (files == null || files.Count == 0) return;

                var target = await Deskdrop.WinUI.Services.DevicePicker.PickAsync(this.XamlRoot, mgr.ConnectedPeers);
                if (target == null && mgr.ConnectedPeers.Count > 0) return; // user cancelled

                foreach (var file in files)
                {
                    // Argument order is (path, name, mime, targetDevice, ...); see the
                    // matching fix note in DashboardWindow.xaml.cs.
                    DaemonClient.SendFilePath(file.Path, file.Name, file.ContentType, target?.device_id);
                }
                DashboardWindow.Current?.NavigateTo("Transfers");
            }
            catch (Exception ex)
            {
                App.HandleError(ex);
            }
        }

        private void OnPingDeviceClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is PeerViewModel peer)
            {
                mgr.SendPushText("__DESKDROP_PING__", peer.device_id);
            }
        }

        private async void OnSendFilesToDeviceClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is PeerViewModel peer)
            {
                try
                {
                    var picker = new Windows.Storage.Pickers.FileOpenPicker();
                    var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(App.MainWindow);
                    WinRT.Interop.InitializeWithWindow.Initialize(picker, hwnd);
                    picker.FileTypeFilter.Add("*");
                    picker.SuggestedStartLocation = Windows.Storage.Pickers.PickerLocationId.Downloads;
                    var files = await picker.PickMultipleFilesAsync();
                    if (files != null && files.Count > 0)
                    {
                        foreach (var file in files)
                        {
                            // Argument order is (path, name, mime, targetDevice, ...); see
                            // the matching fix note in DashboardWindow.xaml.cs.
                            DaemonClient.SendFilePath(file.Path, file.Name, file.ContentType, peer.device_id);
                        }
                        DashboardWindow.Current?.NavigateTo("Transfers");
                    }
                }
                catch (Exception ex)
                {
                    App.HandleError(ex);
                }
            }
        }

        private void OnExploreDeviceClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is PeerViewModel peer)
            {
                mgr.SelectedPeer = peer;
                DashboardWindow.Current?.NavigateTo("DevicePeer");
            }
        }

        private void OnSpeedTestDeviceClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is PeerViewModel peer)
            {
                DaemonClient.StartSpeedTest(peer.device_id, 10);
                DashboardWindow.Current?.NavigateTo("Transfers");
            }
        }

        private void OnViewCameraClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is PeerViewModel peer)
            {
                try
                {
                    var cameraWindow = new CameraPreviewWindow(peer.device_id);
                    cameraWindow.Activate();
                }
                catch (Exception ex)
                {
                    App.HandleError(ex);
                }
            }
        }

        private void OnTransferFilesTapped(object sender, RoutedEventArgs e)
        {
            DashboardWindow.Current?.NavigateTo("Transfers");
        }

        private void OnBrowseDeviceTapped(object sender, RoutedEventArgs e)
        {
            DashboardWindow.Current?.NavigateTo("DevicePeer");
        }

        private void OnClipboardTapped(object sender, RoutedEventArgs e)
        {
            DashboardWindow.Current?.NavigateTo("Clipboard");
        }

        private async void OnSpeedTestTapped(object sender, RoutedEventArgs e)
        {
            var target = await Deskdrop.WinUI.Services.DevicePicker.PickAsync(this.XamlRoot, mgr.ConnectedPeers);
            if (target != null)
            {
                DaemonClient.StartSpeedTest(target.device_id, 10);
                DashboardWindow.Current?.NavigateTo("Transfers");
            }
        }

    }
}
