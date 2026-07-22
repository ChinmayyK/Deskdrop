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
            
            if (Microsoft.UI.Composition.SystemBackdrops.MicaController.IsSupported())
            {
                this.SystemBackdrop = new Microsoft.UI.Xaml.Media.MicaBackdrop();
            }
            else if (Microsoft.UI.Composition.SystemBackdrops.DesktopAcrylicController.IsSupported())
            {
                this.SystemBackdrop = new Microsoft.UI.Xaml.Media.DesktopAcrylicBackdrop();
            }
            
            var timer = new DispatcherTimer { Interval = System.TimeSpan.FromSeconds(10) };
            timer.Tick += (s, e) => { try { timer.Stop(); this.Close(); } catch { } };
            this.Closed += (s, e) => { try { timer.Stop(); } catch { } };
            timer.Start();
        }

        private void BtnAccept_Click(object sender, RoutedEventArgs e)
        {
            try { this.Close(); } catch { }
        }
    }
}

