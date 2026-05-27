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
                    // Push the first file for now (could support multiple in future)
                    string path = files[0];
                    if (File.Exists(path))
                    {
                        System.Threading.Tasks.Task.Run(() => {
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
                        });
                    }
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
