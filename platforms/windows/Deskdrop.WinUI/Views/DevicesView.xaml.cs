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
            this.InitializeComponent();
        }

        private void OnPeerCardTapped(object sender, TappedRoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is PeerViewModel peer)
            {
                mgr.SelectedPeer = peer;
            }
        }

        private void OnDeviceCardPointerMoved(object sender, PointerRoutedEventArgs e)
        {
            if (sender is FrameworkElement element)
            {
                element.CenterPoint = new System.Numerics.Vector3((float)(element.ActualWidth / 2), (float)(element.ActualHeight / 2), 0f);
                element.Scale = new System.Numerics.Vector3(1.02f, 1.02f, 1.0f);
            }
        }

        private void OnDeviceCardPointerExited(object sender, PointerRoutedEventArgs e)
        {
            if (sender is FrameworkElement element)
            {
                element.CenterPoint = new System.Numerics.Vector3((float)(element.ActualWidth / 2), (float)(element.ActualHeight / 2), 0f);
                element.Scale = new System.Numerics.Vector3(1.0f, 1.0f, 1.0f);
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

        private void OnDisconnectClicked(object sender, RoutedEventArgs e)
        {
            if (mgr.SelectedPeer != null)
            {
                mgr.DisconnectPeer(mgr.SelectedPeer.device_id);
            }
        }

        private void OnForgetClicked(object sender, RoutedEventArgs e)
        {
            if (mgr.SelectedPeer != null)
            {
                mgr.ForgetPeer(mgr.SelectedPeer.device_id);
            }
        }

        private void OnScanNearbyClicked(object sender, RoutedEventArgs e)
        {
            mgr.UpdateStateFromDaemon();
        }

        private void OnShowQRCodeClicked(object sender, RoutedEventArgs e)
        {
            try
            {
                var qrWindow = new QRPairingWindow();
                qrWindow.Activate();
            }
            catch (Exception ex)
            {
                App.HandleError(ex);
            }
        }

        private void OnActiveTransfersBannerClicked(object sender, TappedRoutedEventArgs e)
        {
            DashboardWindow.Current?.NavigateTo("Transfers");
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
                            DaemonClient.SendFilePath(peer.device_id, file.Path, file.Name, file.ContentType);
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

        private void OnSpeedTestTapped(object sender, RoutedEventArgs e)
        {
            var firstConnected = mgr.ConnectedPeers.FirstOrDefault();
            if (firstConnected != null)
            {
                DaemonClient.StartSpeedTest(firstConnected.device_id, 10);
            }
            DashboardWindow.Current?.NavigateTo("Transfers");
        }

        private void OnRemoteControlTapped(object sender, TappedRoutedEventArgs e)
        {
            DashboardWindow.Current?.NavigateTo("DevicePeer");
        }

        private void OnSettingsTapped(object sender, TappedRoutedEventArgs e)
        {
            DashboardWindow.Current?.NavigateTo("Settings");
        }
    }
}
