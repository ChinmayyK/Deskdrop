using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using System;
using System.Linq;

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

        private void OnTransferFilesTapped(object sender, TappedRoutedEventArgs e)
        {
            DashboardWindow.Current?.NavigateTo("Transfers");
        }

        private void OnBrowseDeviceTapped(object sender, TappedRoutedEventArgs e)
        {
            DashboardWindow.Current?.NavigateTo("DevicePeer");
        }

        private void OnClipboardTapped(object sender, TappedRoutedEventArgs e)
        {
            DashboardWindow.Current?.NavigateTo("Clipboard");
        }

        private void OnSpeedTestTapped(object sender, TappedRoutedEventArgs e)
        {
            if (mgr.SelectedPeer != null)
            {
                DaemonClient.StartSpeedTest(mgr.SelectedPeer.device_id, 10);
                DashboardWindow.Current?.NavigateTo("Transfers");
            }
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
