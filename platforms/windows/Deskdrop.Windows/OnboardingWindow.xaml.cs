using System.Windows;
using System.Windows.Input;

namespace Deskdrop.Windows
{
    public partial class OnboardingWindow : Window
    {
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
            Close();
        }

        private void BtnShowQRCode_Click(object sender, RoutedEventArgs e)
        {
            new QRCodeWindow(MainWindow.GetLocalIPAddress(), "000000").ShowDialog();
            Close();
        }
    }
}
