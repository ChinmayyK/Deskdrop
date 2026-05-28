using System;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media.Imaging;
using System.Text.Json;

namespace Deskdrop.Windows
{
    public partial class CameraPreviewWindow : Window
    {
        private bool _isPolling;
        private string _peerId;

        public CameraPreviewWindow(string peerId)
        {
            InitializeComponent();
            _peerId = peerId;
            StartPolling();
        }

        private void StartPolling()
        {
            _isPolling = true;
            Task.Run(async () =>
            {
                while (_isPolling)
                {
                    try
                    {
                        var resp = DaemonClient.LatestCameraFrame(_peerId);
                        if (resp != null && resp.RootElement.TryGetProperty("data", out var dataProp))
                        {
                            if (dataProp.TryGetProperty("frame_base64", out var frameProp))
                            {
                                var base64 = frameProp.GetString();
                                if (!string.IsNullOrEmpty(base64))
                            {
                                var bytes = Convert.FromBase64String(base64);
                                await Dispatcher.InvokeAsync(() =>
                                {
                                    StatusText.Visibility = Visibility.Collapsed;
                                    using var ms = new MemoryStream(bytes);
                                    var bitmap = new BitmapImage();
                                    bitmap.BeginInit();
                                    bitmap.CacheOption = BitmapCacheOption.OnLoad;
                                    bitmap.StreamSource = ms;
                                    bitmap.EndInit();
                                    bitmap.Freeze();
                                    CameraImage.Source = bitmap;
                                });
                            }
                            }
                        }
                    }
                    catch { }

                    await Task.Delay(33); // ~30 fps max
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

        private void Window_Closing(object sender, System.ComponentModel.CancelEventArgs e)
        {
            _isPolling = false;
        }
    }
}
