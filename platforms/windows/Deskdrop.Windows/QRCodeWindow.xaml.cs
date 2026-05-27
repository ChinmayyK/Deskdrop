using System;
using System.IO;
using System.Windows;
using System.Windows.Media.Imaging;
using QRCoder;

namespace Deskdrop.Windows
{
    public partial class QRCodeWindow : Window
    {
        private System.Windows.Threading.DispatcherTimer _timer;

        public QRCodeWindow(string payload, string pinCode)
        {
            InitializeComponent();
            TxtPin.Text = pinCode;
            GenerateQRCode(payload);

            _timer = new System.Windows.Threading.DispatcherTimer();
            _timer.Interval = TimeSpan.FromSeconds(1);
            _timer.Tick += Timer_Tick;
            _timer.Start();
        }

        private void Timer_Tick(object? sender, EventArgs e)
        {
            System.Threading.Tasks.Task.Run(() =>
            {
                if (DaemonClient.IsDaemonRunning())
                {
                    try
                    {
                        var state = DaemonClient.Status();
                        if (state != null && state.RootElement.TryGetProperty("data", out var data) && data.TryGetProperty("peer_count", out var pc))
                        {
                            if (pc.GetInt32() > 0)
                            {
                                Dispatcher.Invoke(() =>
                                {
                                    _timer.Stop();
                                    this.Close();
                                });
                            }
                        }
                    }
                    catch { }
                }
            });
        }

        private void GenerateQRCode(string payload)
        {
            try
            {
                using (var qrGenerator = new QRCodeGenerator())
                using (var qrCodeData = qrGenerator.CreateQrCode(payload, QRCodeGenerator.ECCLevel.Q))
                {
                    var qrCode = new PngByteQRCode(qrCodeData);
                    byte[] qrBytes = qrCode.GetGraphic(20);
                    using (var ms = new MemoryStream(qrBytes))
                    {
                        var bitmapImage = new BitmapImage();
                        bitmapImage.BeginInit();
                        bitmapImage.CacheOption = BitmapCacheOption.OnLoad;
                        bitmapImage.StreamSource = ms;
                        bitmapImage.EndInit();
                        ImgQRCode.Source = bitmapImage;
                    }
                }
            }
            catch (Exception ex)
            {
                System.Windows.MessageBox.Show("Failed to generate QR Code: " + ex.Message);
            }
        }

        protected override void OnClosed(EventArgs e)
        {
            base.OnClosed(e);
            _timer?.Stop();
        }
    }
}
