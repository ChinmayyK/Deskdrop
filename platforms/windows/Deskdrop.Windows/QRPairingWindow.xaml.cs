using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Sockets;
using System.Text.Json;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Media.Imaging;
using QRCoder;

namespace Deskdrop.Windows
{
    public partial class QRPairingWindow : Window
    {
        public QRPairingWindow()
        {
            InitializeComponent();
            Loaded += QRPairingWindow_Loaded;
        }

        private async void QRPairingWindow_Loaded(object sender, RoutedEventArgs e)
        {
            await Task.Run(() =>
            {
                try
                {
                    var state = DaemonClient.Status();
                    if (state == null) return;

                    var root = state.RootElement;
                    var name = root.TryGetProperty("device_name", out var nProp) && nProp.ValueKind == JsonValueKind.String 
                                ? nProp.GetString() : Environment.MachineName;
                    
                    var fp = root.TryGetProperty("fingerprint_display", out var fProp) 
                                ? fProp.GetString() : "";

                    var ip = root.TryGetProperty("bind_ip", out var ipProp) ? ipProp.GetString() : GetLocalIPAddress();
                    var port = root.TryGetProperty("bind_port", out var pProp) && pProp.TryGetInt32(out var p) ? p : 47823;

                    var url = $"deskdrop://pair?name={Uri.EscapeDataString(name ?? "")}&ip={ip}&port={port}&fingerprint={fp}";

                    var qrGenerator = new QRCodeGenerator();
                    var qrCodeData = qrGenerator.CreateQrCode(url, QRCodeGenerator.ECCLevel.M);
                    var qrCode = new QRCode(qrCodeData);
                    using var bitmap = qrCode.GetGraphic(10);
                    
                    using var ms = new MemoryStream();
                    bitmap.Save(ms, ImageFormat.Png);
                    ms.Position = 0;

                    Dispatcher.Invoke(() =>
                    {
                        var bmpImage = new BitmapImage();
                        bmpImage.BeginInit();
                        bmpImage.CacheOption = BitmapCacheOption.OnLoad;
                        bmpImage.StreamSource = ms;
                        bmpImage.EndInit();
                        ImgQRCode.Source = bmpImage;
                    });
                }
                catch (Exception ex)
                {
                    Dispatcher.Invoke(() => System.Windows.MessageBox.Show("Could not generate QR Code: " + ex.Message));
                }
            });
        }

        private static string GetLocalIPAddress()
        {
            var host = Dns.GetHostEntry(Dns.GetHostName());
            foreach (var ip in host.AddressList)
            {
                if (ip.AddressFamily == AddressFamily.InterNetwork)
                {
                    // Ignore docker/wsl internal subnets if possible, but returning first IPv4 is standard
                    return ip.ToString();
                }
            }
            return "127.0.0.1";
        }
    }
}
