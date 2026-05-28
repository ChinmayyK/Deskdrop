using System;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Sockets;
using System.Text.Json;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media.Imaging;
using QRCoder;

namespace Deskdrop.Windows
{
    public partial class QRPairingWindow : Window
    {
        private int _initialPeerCount;

        public QRPairingWindow()
        {
            InitializeComponent();
            _initialPeerCount = DeskdropStore.Shared.Peers.Count;
            Loaded += QRPairingWindow_Loaded;
            DeskdropStore.Shared.Peers.CollectionChanged += Peers_CollectionChanged;
        }

        private void Peers_CollectionChanged(object? sender, System.Collections.Specialized.NotifyCollectionChangedEventArgs e)
        {
            if (DeskdropStore.Shared.Peers.Count > _initialPeerCount)
            {
                Dispatcher.Invoke(() => this.Close());
            }
        }

        protected override void OnClosed(EventArgs e)
        {
            base.OnClosed(e);
            DeskdropStore.Shared.Peers.CollectionChanged -= Peers_CollectionChanged;
        }

        private async void QRPairingWindow_Loaded(object sender, RoutedEventArgs e)
        {
            await Task.Run(() =>
            {
                try
                {
                    var state = DaemonClient.Status();
                    if (state == null) return;

                    JsonDocument? settings = null;
                    try { settings = DaemonClient.Send(new { cmd = "get_settings" }); } catch { }

                    var root = state.RootElement.TryGetProperty("data", out var d) ? d : state.RootElement;
                    var name = settings != null && settings.RootElement.TryGetProperty("data", out var sData) && sData.TryGetProperty("device_name", out var nProp) && nProp.ValueKind == JsonValueKind.String 
                                ? nProp.GetString() : Environment.MachineName;
                    
                    var fp = root.TryGetProperty("local_fingerprint", out var fProp) 
                                ? fProp.GetString() : "";

                    var ip = root.TryGetProperty("bind_ip", out var ipProp) ? ipProp.GetString() : GetLocalIPAddress();
                    var port = root.TryGetProperty("bind_port", out var pProp) && pProp.TryGetInt32(out var p) ? p : 47823;

                    var url = $"deskdrop://pair?name={Uri.EscapeDataString(name ?? "")}&ip={ip}&port={port}&fingerprint={fp}";

                    var qrGenerator = new QRCodeGenerator();
                    var qrCodeData = qrGenerator.CreateQrCode(url, QRCodeGenerator.ECCLevel.M);
                    var qrCode = new PngByteQRCode(qrCodeData);
                    byte[] qrCodeImage = qrCode.GetGraphic(10);
                    
                    using var ms = new MemoryStream(qrCodeImage);

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

        private void Window_MouseDown(object sender, MouseButtonEventArgs e)
        {
            if (e.ChangedButton == MouseButton.Left)
                this.DragMove();
        }

        private void BtnClose_Click(object sender, RoutedEventArgs e)
        {
            this.Close();
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
