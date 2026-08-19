using System;
using System.IO;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Windowing;
using Microsoft.UI.Composition.SystemBackdrops;
using QRCoder;
using Windows.Storage.Streams;

namespace Deskdrop.WinUI
{
    public sealed partial class QRPairingWindow : Window
    {
        public QRPairingWindow()
        {
            this.InitializeComponent();
            ExtendsContentIntoTitleBar = true;
            SetTitleBar(AppTitleBar);

            // Resize the window
            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            var windowId = Microsoft.UI.Win32Interop.GetWindowIdFromWindow(hwnd);
            var appWindow = Microsoft.UI.Windowing.AppWindow.GetFromWindowId(windowId);
            appWindow.Resize(new Windows.Graphics.SizeInt32(280, 340));

            // Generate the QR Code dynamically with local IP (Legacy Format for compatibility)
            try
            {
                var name = System.Net.WebUtility.UrlEncode(System.Environment.MachineName);
                var ip = System.Net.Dns.GetHostEntry(System.Net.Dns.GetHostName()).AddressList.FirstOrDefault(x => x.AddressFamily == System.Net.Sockets.AddressFamily.InterNetwork)?.ToString();
                
                if (!string.IsNullOrEmpty(ip))
                {
                    string uri = $"deskdrop://pair?ip={ip}&port=47823&name={name}";
                    _ = GenerateQRCodeAsync(uri);
                }
                else
                {
                    System.Diagnostics.Debug.WriteLine("Failed to fetch local IP for QR.");
                }
            }
            catch (Exception ex)
            {
                System.Diagnostics.Debug.WriteLine($"Error preparing QR data: {ex.Message}");
            }
        }

        private async Task GenerateQRCodeAsync(string payload)
        {
            try
            {
                using var qrGenerator = new QRCodeGenerator();
                using var qrCodeData = qrGenerator.CreateQrCode(payload, QRCodeGenerator.ECCLevel.Q);
                using var qrCode = new PngByteQRCode(qrCodeData);
                
                byte[] qrCodeBytes = qrCode.GetGraphic(20);
                
                using var stream = new InMemoryRandomAccessStream();
                using var writer = new DataWriter(stream.GetOutputStreamAt(0));
                writer.WriteBytes(qrCodeBytes);
                await writer.StoreAsync();

                var bitmapImage = new BitmapImage();
                await bitmapImage.SetSourceAsync(stream);
                
                QrCodeImage.Source = bitmapImage;
            }
            catch (Exception ex)
            {
                System.Diagnostics.Debug.WriteLine($"Failed to generate QR Code: {ex.Message}");
            }
        }
    }
}


