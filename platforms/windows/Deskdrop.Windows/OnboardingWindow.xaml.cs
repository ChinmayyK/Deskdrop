using System;
using System.Linq;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Shapes;
using System.Windows.Controls;

namespace Deskdrop.Windows
{
    public partial class OnboardingWindow : Window
    {
        public bool Success { get; private set; } = false;

        private int _currentStep = 0;
        private string? _selectedPeerId = null;
        private DateTime _sessionStartTime = DateTime.Now;

        public OnboardingWindow()
        {
            InitializeComponent();
            Loaded += OnboardingWindow_Loaded;
            Unloaded += OnboardingWindow_Unloaded;
            DeskdropStore.Shared.Peers.CollectionChanged += Peers_CollectionChanged;
            DeskdropStore.Shared.PropertyChanged += Store_PropertyChanged;
        }

        private void OnboardingWindow_Loaded(object sender, RoutedEventArgs e)
        {
            UpdateUIForStep();
            
            // Re-subscribe to peers' PropertyChanged because the collection might not change, but individual items will.
            foreach (var peer in DeskdropStore.Shared.Peers)
            {
                peer.PropertyChanged -= Peer_PropertyChanged;
                peer.PropertyChanged += Peer_PropertyChanged;
            }
        }

        private void OnboardingWindow_Unloaded(object sender, RoutedEventArgs e)
        {
            DeskdropStore.Shared.Peers.CollectionChanged -= Peers_CollectionChanged;
            DeskdropStore.Shared.PropertyChanged -= Store_PropertyChanged;
            foreach (var peer in DeskdropStore.Shared.Peers)
            {
                peer.PropertyChanged -= Peer_PropertyChanged;
            }
        }

        private void Store_PropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
        {
            if (e.PropertyName == nameof(DeskdropStore.ActivityFeed))
            {
                Dispatcher.Invoke(() => UpdateUIForStep());
            }
        }

        private void Peers_CollectionChanged(object? sender, System.Collections.Specialized.NotifyCollectionChangedEventArgs e)
        {
            if (e.NewItems != null)
            {
                foreach (PeerViewModel peer in e.NewItems)
                {
                    peer.PropertyChanged += Peer_PropertyChanged;
                }
            }
            if (e.OldItems != null)
            {
                foreach (PeerViewModel peer in e.OldItems)
                {
                    peer.PropertyChanged -= Peer_PropertyChanged;
                }
            }
            Dispatcher.Invoke(() => UpdateUIForStep());
        }

        private void Peer_PropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
        {
            Dispatcher.Invoke(() => UpdateUIForStep());
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            DragMove();
        }

        private PeerViewModel? GetSelectedPeer()
        {
            return DeskdropStore.Shared.Peers.FirstOrDefault(p => p.device_id == _selectedPeerId);
        }

        private void ComputeCurrentStep()
        {
            if (_selectedPeerId == null)
            {
                _currentStep = 0;
                return;
            }

            var peer = GetSelectedPeer();
            if (peer == null)
            {
                _currentStep = 0;
                return;
            }

            if (!peer.is_trusted)
            {
                _currentStep = 1;
                return;
            }

            // We are trusted. Did we receive anything recently?
            bool hasRecentActivity = DeskdropStore.Shared.ActivityFeed.Any(a => a.source == peer.device_id && DateTimeOffset.FromUnixTimeSeconds((long)a.timestamp).UtcDateTime > _sessionStartTime.ToUniversalTime());
            
            if (hasRecentActivity)
            {
                _currentStep = 3;
                return;
            }

            _currentStep = 2;
        }

        private void UpdateUIForStep()
        {
            ComputeCurrentStep();

            // Update Pagination
            var brandElectric = (SolidColorBrush)FindResource("BrandElectric");
            var strokeSoft = (SolidColorBrush)FindResource("StrokeSoft");
            Dot0.Fill = _currentStep == 0 ? brandElectric : strokeSoft;
            Dot1.Fill = _currentStep == 1 ? brandElectric : strokeSoft;
            Dot2.Fill = _currentStep == 2 ? brandElectric : strokeSoft;
            Dot3.Fill = _currentStep == 3 ? brandElectric : strokeSoft;

            Step1View.Visibility = _currentStep == 0 ? Visibility.Visible : Visibility.Collapsed;
            Step2View.Visibility = _currentStep == 1 ? Visibility.Visible : Visibility.Collapsed;
            Step3View.Visibility = _currentStep == 2 ? Visibility.Visible : Visibility.Collapsed;
            Step4View.Visibility = _currentStep == 3 ? Visibility.Visible : Visibility.Collapsed;

            // Footer
            if (_currentStep == 0)
            {
                BtnFooterLeft.Content = "Skip for now";
                BtnFooterLeft.Visibility = Visibility.Visible;
                BtnFooterRight.Visibility = Visibility.Collapsed;
            }
            else if (_currentStep > 0 && _currentStep < 3)
            {
                BtnFooterLeft.Content = "Cancel";
                BtnFooterLeft.Visibility = Visibility.Visible;
                BtnFooterRight.Visibility = Visibility.Collapsed;
            }
            else if (_currentStep == 3)
            {
                BtnFooterLeft.Visibility = Visibility.Collapsed;
                BtnFooterRight.Visibility = Visibility.Visible;
            }

            // Step 1 Specifics
            if (_currentStep == 0)
            {
                if (DeskdropStore.Shared.Peers.Count == 0)
                {
                    ScanningPanel.Visibility = Visibility.Visible;
                    PeersList.Visibility = Visibility.Collapsed;
                }
                else
                {
                    ScanningPanel.Visibility = Visibility.Collapsed;
                    PeersList.Visibility = Visibility.Visible;
                    PeersList.ItemsSource = DeskdropStore.Shared.Peers;
                }
            }

            // Step 2 Specifics
            if (_currentStep == 1)
            {
                var peer = GetSelectedPeer();
                if (peer != null)
                {
                    if (!string.IsNullOrEmpty(peer.pairingPin))
                    {
                        ConnectingPanel.Visibility = Visibility.Collapsed;
                        PinPanel.Visibility = Visibility.Visible;
                        PinText.Text = peer.pairingPin;
                        VerifyDeviceNameText.Text = $"Ensure this matches the code on {peer.friendly_name}:";
                        
                        PairingActionsPanel.Visibility = peer.pairingRequested ? Visibility.Visible : Visibility.Collapsed;
                    }
                    else
                    {
                        ConnectingPanel.Visibility = Visibility.Visible;
                        PinPanel.Visibility = Visibility.Collapsed;
                        ConnectingText.Text = $"Connecting to {peer.friendly_name}...";
                    }
                }
            }
        }

        private void PeerButton_Click(object sender, RoutedEventArgs e)
        {
            if (sender is System.Windows.Controls.Button btn && btn.DataContext is PeerViewModel peer)
            {
                _selectedPeerId = peer.device_id;
                DeskdropStore.Shared.ConnectAndPair(peer.device_id);
                UpdateUIForStep();
            }
        }

        private void BtnShowQRCode_Click(object sender, RoutedEventArgs e)
        {
            new QRPairingWindow().ShowDialog();
            UpdateUIForStep();
        }

        private void BtnDecline_Click(object sender, RoutedEventArgs e)
        {
            if (_selectedPeerId != null)
            {
                DeskdropStore.Shared.RespondToPairing(_selectedPeerId, false);
                _selectedPeerId = null;
                UpdateUIForStep();
            }
        }

        private void BtnTrust_Click(object sender, RoutedEventArgs e)
        {
            if (_selectedPeerId != null)
            {
                DeskdropStore.Shared.RespondToPairing(_selectedPeerId, true);
            }
        }

        private void BtnSendSample_Click(object sender, RoutedEventArgs e)
        {
            if (_selectedPeerId != null)
            {
                DeskdropStore.Shared.SendPushText("Hello from Windows", _selectedPeerId);
            }
        }

        private void BtnFooterLeft_Click(object sender, RoutedEventArgs e)
        {
            if (_currentStep == 0)
            {
                var res = System.Windows.MessageBox.Show("Are you sure you want to skip pairing? You won't be able to drop files to other devices until you pair.", "Skip Setup", System.Windows.MessageBoxButton.YesNo, System.Windows.MessageBoxImage.Question);
                if (res == System.Windows.MessageBoxResult.Yes)
                {
                    Success = true;
                    Close();
                }
            }
            else
            {
                _selectedPeerId = null;
                UpdateUIForStep();
            }
        }

        private void BtnFooterRight_Click(object sender, RoutedEventArgs e)
        {
            Success = true;
            Close();
        }
    }
}
