using System;
using System.IO;
using System.Linq;
using System.Windows;
using System.Windows.Media.Animation;
using System.IO.Compression;

namespace Deskdrop.Windows.UI
{
    public partial class EdgeDropWindow : Window
    {
        private readonly ClipboardManager _clipboardManager;
        private bool _isExpanded = false;

        public EdgeDropWindow(ClipboardManager clipboardManager)
        {
            InitializeComponent();
            _clipboardManager = clipboardManager;
        }

        private void Window_Loaded(object sender, RoutedEventArgs e)
        {
            Reposition(false);
        }

        public void Reposition(bool expanded)
        {
            _isExpanded = expanded;
            var cursorPosition = System.Windows.Forms.Cursor.Position;
            var screen = System.Windows.Forms.Screen.FromPoint(cursorPosition);
            var workArea = screen.WorkingArea;

            var source = PresentationSource.FromVisual(this);
            double scaleX = source?.CompositionTarget?.TransformToDevice.M11 ?? 1.0;
            double scaleY = source?.CompositionTarget?.TransformToDevice.M22 ?? 1.0;

            double winWidth = expanded ? 280 : 16;
            double winHeight = expanded ? 240 : 200;

            Width = winWidth;
            Height = winHeight;
            Left = workArea.Left / scaleX;
            Top = (workArea.Top / scaleY) + ((workArea.Height / scaleY) - winHeight) / 2.0;

            if (expanded)
            {
                RestingSliver.Visibility = Visibility.Collapsed;
                ExpandedCard.Visibility = Visibility.Visible;
                var fadeIn = new DoubleAnimation(0.0, 1.0, TimeSpan.FromMilliseconds(200));
                ExpandedCard.BeginAnimation(UIElement.OpacityProperty, fadeIn);

                var connected = DeskdropStore.Shared.Peers.Where(p => p.IsConnected).ToList();
                ExpandedSubText.Text = connected.Count > 0
                    ? $"Instant transfer to {string.Join(", ", connected.Select(c => c.friendly_name))}"
                    : "Sending to active mesh";
            }
            else
            {
                RestingSliver.Visibility = Visibility.Visible;
                ExpandedCard.Visibility = Visibility.Collapsed;
                ExpandedCard.Opacity = 0;
            }
        }

        private void Window_DragEnter(object sender, DragEventArgs e)
        {
            if (e.Data.GetDataPresent(DataFormats.FileDrop))
            {
                e.Effects = DragDropEffects.Copy;
                if (!_isExpanded)
                {
                    Reposition(true);
                }
            }
            else
            {
                e.Effects = DragDropEffects.None;
            }
        }

        private void Window_DragLeave(object sender, DragEventArgs e)
        {
            Dispatcher.BeginInvoke(new Action(() => {
                var pos = System.Windows.Forms.Cursor.Position;
                var source = PresentationSource.FromVisual(this);
                double scaleX = source?.CompositionTarget?.TransformToDevice.M11 ?? 1.0;
                double scaleY = source?.CompositionTarget?.TransformToDevice.M22 ?? 1.0;
                double left = Left * scaleX;
                double top = Top * scaleY;
                double right = (Left + Width) * scaleX;
                double bottom = (Top + Height) * scaleY;

                if (pos.X < left || pos.X > right || pos.Y < top || pos.Y > bottom)
                {
                    Reposition(false);
                }
            }), System.Windows.Threading.DispatcherPriority.Background);
        }

        private void Window_Drop(object sender, DragEventArgs e)
        {
            Reposition(false);
            if (e.Data.GetDataPresent(DataFormats.FileDrop))
            {
                string[] files = (string[])e.Data.GetData(DataFormats.FileDrop);
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
                                        NotificationHelper.ShowToast("Deskdrop Portal", $"Instant sent {Path.GetFileName(path)}!");
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
                                using (var archive = ZipFile.Open(tempZip, ZipArchiveMode.Create))
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
                                    NotificationHelper.ShowToast("Deskdrop Portal", $"Instant sent {files.Length} files!");
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
        }
    }
}
