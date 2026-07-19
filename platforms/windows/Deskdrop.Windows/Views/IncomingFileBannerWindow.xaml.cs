using System;
using System.Windows;
using System.Windows.Media.Animation;

namespace Deskdrop.Windows
{
    public partial class IncomingFileBannerWindow : Window
    {
        public IncomingFileBannerWindow(string fileName, string senderName)
        {
            InitializeComponent();
            TxtFileName.Text = fileName;
            TxtSenderName.Text = $"from {senderName}";
            
            // Position at top center
            var workArea = SystemParameters.WorkArea;
            Left = workArea.Left + (workArea.Width - Width) / 2;
            Top = workArea.Top + 20;
            
            // Auto close after 5 seconds
            var timer = new System.Windows.Threading.DispatcherTimer { Interval = TimeSpan.FromSeconds(5) };
            timer.Tick += (s, e) =>
            {
                timer.Stop();
                CloseWithAnimation();
            };
            timer.Start();
        }

        private void BtnAccept_Click(object sender, RoutedEventArgs e)
        {
            // Just close it for now, it's just a notification
            CloseWithAnimation();
        }

        private void CloseWithAnimation()
        {
            var anim = new DoubleAnimation(0, TimeSpan.FromSeconds(0.2));
            anim.Completed += (s, e) => Close();
            BeginAnimation(OpacityProperty, anim);
        }
    }
}
