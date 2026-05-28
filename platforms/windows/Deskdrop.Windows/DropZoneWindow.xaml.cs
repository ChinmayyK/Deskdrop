using System;
using System.IO;
using System.Windows;

namespace Deskdrop.Windows
{
    public partial class DropZoneWindow : Window
    {
        private readonly ClipboardManager _clipboardManager;

        public DropZoneWindow(ClipboardManager clipboardManager)
        {
            InitializeComponent();
            _clipboardManager = clipboardManager;
        }

        private void Window_Loaded(object sender, RoutedEventArgs e)
        {
            var cursorPosition = System.Windows.Forms.Cursor.Position;
            var screen = System.Windows.Forms.Screen.FromPoint(cursorPosition);
            var workArea = screen.WorkingArea;

            var source = PresentationSource.FromVisual(this);
            double scaleX = source?.CompositionTarget?.TransformToDevice.M11 ?? 1.0;
            double scaleY = source?.CompositionTarget?.TransformToDevice.M22 ?? 1.0;

            WindowState = WindowState.Normal;
            Left = workArea.Left / scaleX;
            Top = workArea.Top / scaleY;
            Width = workArea.Width / scaleX;
            Height = workArea.Height / scaleY;
            WindowState = WindowState.Maximized;
        }

        private void Window_DragEnter(object sender, System.Windows.DragEventArgs e)
        {
            if (e.Data.GetDataPresent(System.Windows.DataFormats.FileDrop))
            {
                e.Effects = System.Windows.DragDropEffects.Copy;
                DropBorder.Opacity = 1.0;
            }
            else
            {
                e.Effects = System.Windows.DragDropEffects.None;
            }
        }

        private void Window_DragLeave(object sender, System.Windows.DragEventArgs e)
        {
            DropBorder.Opacity = 0.5;
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
    }
}
