using System;
using System.Media;
using System.Windows;
using System.Windows.Threading;

namespace Deskdrop.Windows
{
    public partial class IncomingCallBannerWindow : Window
    {
        public event EventHandler? CallAccepted;
        public event EventHandler? CallDeclined;
        
        private readonly DispatcherTimer _timer;

        public IncomingCallBannerWindow(string callerName)
        {
            InitializeComponent();
            
            TxtCallerName.Text = string.IsNullOrEmpty(callerName) ? "Incoming Call..." : callerName;
            TxtInitials.Text = string.IsNullOrEmpty(callerName) ? "?" : callerName.Substring(0, 1).ToUpper();

            _timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(2) };
            _timer.Tick += (s, e) => SystemSounds.Asterisk.Play();
            _timer.Start();

            // Position at top center
            var workArea = SystemParameters.WorkArea;
            Left = workArea.Left + (workArea.Width - Width) / 2;
            Top = workArea.Top + 60;
        }

        private void BtnAccept_Click(object sender, RoutedEventArgs e)
        {
            _timer.Stop();
            CallAccepted?.Invoke(this, EventArgs.Empty);
            Close();
        }

        private void BtnDecline_Click(object sender, RoutedEventArgs e)
        {
            _timer.Stop();
            CallDeclined?.Invoke(this, EventArgs.Empty);
            Close();
        }

        protected override void OnClosed(EventArgs e)
        {
            _timer.Stop();
            base.OnClosed(e);
        }
    }
}
