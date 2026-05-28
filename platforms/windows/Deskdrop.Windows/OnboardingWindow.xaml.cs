using System.Windows;
using System.Windows.Input;

namespace Deskdrop.Windows
{
    public partial class OnboardingWindow : Window
    {
        public bool Success { get; private set; } = false;

        public OnboardingWindow()
        {
            InitializeComponent();
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            DragMove();
        }

        private void BtnClose_Click(object sender, RoutedEventArgs e)
        {
            Close();
        }

        private void BtnSkip_Click(object sender, RoutedEventArgs e)
        {
            var res = System.Windows.MessageBox.Show("Are you sure you want to skip pairing? You won't be able to drop files to other devices until you pair.", "Skip Setup", System.Windows.MessageBoxButton.YesNo, System.Windows.MessageBoxImage.Question);
            if (res == System.Windows.MessageBoxResult.Yes)
            {
                Success = true;
                Close();
            }
        }

        private void BtnShowQRCode_Click(object sender, RoutedEventArgs e)
        {
            new QRPairingWindow().ShowDialog();
            
            // Check if we paired successfully
            if (DeskdropStore.Shared.Peers.Count > 0)
            {
                IntroView.Visibility = Visibility.Collapsed;
                DoneView.Visibility = Visibility.Visible;
            }
        }

        private void BtnFinish_Click(object sender, RoutedEventArgs e)
        {
            Success = true;
            Close();
        }
    }
}
