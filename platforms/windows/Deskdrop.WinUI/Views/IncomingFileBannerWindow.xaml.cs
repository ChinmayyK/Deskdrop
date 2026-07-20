using Microsoft.UI.Xaml;

namespace Deskdrop.WinUI.Views
{
    public partial class IncomingFileBannerWindow : Window
    {
        public IncomingFileBannerWindow(string fileName, string senderName) 
        {  
            this.InitializeComponent();
            TxtFileName.Text = "Receiving " + fileName + "...";
            TxtSenderName.Text = "from " + senderName;
            
            ExtendsContentIntoTitleBar = true;
            
            var timer = new DispatcherTimer { Interval = System.TimeSpan.FromSeconds(10) };
            timer.Tick += (s, e) => { timer.Stop(); this.Close(); };
            timer.Start();
        }

        private void BtnAccept_Click(object sender, RoutedEventArgs e)
        {
            this.Close();
        }
    }
}

