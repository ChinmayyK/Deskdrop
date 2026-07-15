using System;
using System.IO;
using System.Linq;
using System.Windows;
using System.Windows.Media;
using System.IO.Compression;

namespace Deskdrop.Windows
{
    public partial class DropZoneWindow : Window
    {
        private readonly ClipboardManager _clipboardManager;

        public DropZoneWindow(ClipboardManager clipboardManager)
        {
            InitializeComponent();
            _clipboardManager = clipboardManager;
            UpdateStatus();
        }

        private void UpdateStatus(bool isTargeted = false)
        {
            var connected = DeskdropStore.Shared.Peers.Where(p => p.IsConnected).ToList();
            if (StatusPillCircle != null && StatusPillText != null)
            {
                if (connected.Count > 0)
                {
                    StatusPillCircle.Fill = new SolidColorBrush((Color)ColorConverter.ConvertFromString("#34C759"));
                    StatusPillText.Text = $"{connected.Count} Device{(connected.Count == 1 ? "" : "s")} Ready";
                }
                else
                {
                    StatusPillCircle.Fill = new SolidColorBrush((Color)ColorConverter.ConvertFromString("#FF9500"));
                    StatusPillText.Text = "Searching Peers...";
                }
            }

            if (DropTitleText != null && DropSubText != null)
            {
                if (isTargeted)
                {
                    DropTitleText.Text = "Release to Broadcast ✨";
                    DropSubText.Text = connected.Count > 0
                        ? $"Sending to {string.Join(", ", connected.Select(c => c.friendly_name))}"
                        : "Broadcasting to active mesh";
                }
                else
                {
                    DropTitleText.Text = "Drop to Broadcast";
                    DropSubText.Text = connected.Count > 0
                        ? $"Instant transfer to {(connected.Count == 1 ? connected[0].friendly_name : $"{connected.Count} connected devices")}"
                        : "Wireless transfer to nearby devices";
                }
            }
        }

        private void Window_Loaded(object sender, RoutedEventArgs e)
        {
            var cursorPosition = System.Windows.Forms.Cursor.Position;
            var screen = System.Windows.Forms.Screen.FromPoint(cursorPosition);
            var workArea = screen.WorkingArea;

            var source = PresentationSource.FromVisual(this);
            double scaleX = source?.CompositionTarget?.TransformToDevice.M11 ?? 1.0;
            double scaleY = source?.CompositionTarget?.TransformToDevice.M22 ?? 1.0;

            double winWidth = 320;
            double winHeight = 216;
            WindowState = WindowState.Normal;
            Width = winWidth;
            Height = winHeight;
            Left = (workArea.Left / scaleX) + ((workArea.Width / scaleX) - winWidth) / 2.0;
            Top = (workArea.Bottom / scaleY) - winHeight - 80;
            UpdateStatus(false);
        }

        private void Window_DragEnter(object sender, System.Windows.DragEventArgs e)
        {
            if (e.Data.GetDataPresent(System.Windows.DataFormats.FileDrop))
            {
                e.Effects = System.Windows.DragDropEffects.Copy;
                DropGrid.Opacity = 1.0;
                UpdateStatus(true);
            }
            else
            {
                e.Effects = System.Windows.DragDropEffects.None;
            }
        }

        private void Window_DragLeave(object sender, System.Windows.DragEventArgs e)
        {
            DropGrid.Opacity = 0.95;
            UpdateStatus(false);
        }

        private void Window_Drop(object sender, System.Windows.DragEventArgs e)
        {
            if (e.Data.GetDataPresent(System.Windows.DataFormats.FileDrop))
            {
                string[] files = (string[])e.Data.GetData(System.Windows.DataFormats.FileDrop);
                if (files != null && files.Length > 0)
                {
                    System.Threading.Tasks.Task.Run(() => {
                        if (files.Length == 1)
                        {
                            var path = files[0];
                            if (File.Exists(path))
                            {
                                try {
                                    _clipboardManager.PushFile(path);
                                    System.Windows.Application.Current.Dispatcher.Invoke(() => {
                                        NotificationHelper.ShowToast("Deskdrop", $"Sending {Path.GetFileName(path)}...");
                                    });
                                } catch (Exception ex) {
                                    System.Windows.Application.Current.Dispatcher.Invoke(() => {
                                        NotificationHelper.ShowToast("Deskdrop Error", $"Failed to send file: {ex.Message}");
                                    });
                                }
                            }
                        }
                        else
                        {
                            try {
                                var tempZip = Path.Combine(Path.GetTempPath(), $"deskdrop_batch_{DateTime.Now:yyyyMMddHHmmss}.zip");
                                using (var archive = System.IO.Compression.ZipFile.Open(tempZip, System.IO.Compression.ZipArchiveMode.Create))
                                {
                                    foreach (string path in files)
                                    {
                                        if (File.Exists(path))
                                        {
                                            archive.CreateEntryFromFile(path, Path.GetFileName(path));
                                        }
                                    }
                                }
                                _clipboardManager.PushFile(tempZip);
                                System.Windows.Application.Current.Dispatcher.Invoke(() => {
                                    NotificationHelper.ShowToast("Deskdrop", $"Sending {files.Length} files as batch...");
                                });
                            } catch (Exception ex) {
                                System.Windows.Application.Current.Dispatcher.Invoke(() => {
                                    NotificationHelper.ShowToast("Deskdrop Error", $"Failed to zip and send files: {ex.Message}");
                                });
                            }
                        }
                    });
                }
            }
            Close();
        }

        private void Window_MouseLeftButtonDown(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            Close();
        }

        private void CloseButton_Click(object sender, RoutedEventArgs e)
        {
            Close();
        }
    }
}
