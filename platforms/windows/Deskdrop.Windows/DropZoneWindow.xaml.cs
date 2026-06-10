using System;
using System.IO;
using System.Windows;
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
                DropGrid.Opacity = 1.0;
            }
            else
            {
                e.Effects = System.Windows.DragDropEffects.None;
            }
        }

        private void Window_DragLeave(object sender, System.Windows.DragEventArgs e)
        {
            DropGrid.Opacity = 0.5;
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
                            if (File.Exists(path) || Directory.Exists(path))
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
                                _clipboardManager.PushFiles(files, "deskdrop_bundle.zip");
                                System.Windows.Application.Current.Dispatcher.Invoke(() => {
                                    NotificationHelper.ShowToast("Deskdrop", $"Sending {files.Length} files as batch...");
                                });
                            } catch (Exception ex) {
                                System.Windows.Application.Current.Dispatcher.Invoke(() => {
                                    NotificationHelper.ShowToast("Deskdrop Error", $"Failed to send files: {ex.Message}");
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
