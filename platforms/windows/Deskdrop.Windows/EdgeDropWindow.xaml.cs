using System;
using System.Windows;
using System.Windows.Media.Animation;
using System.Windows.Media;

namespace Deskdrop.Windows
{
    public partial class EdgeDropWindow : Window
    {
        private TrayApp _trayApp;
        private bool _isLeftEdge;
        private double _screenWidth;
        private double _screenHeight;

        public EdgeDropWindow(TrayApp trayApp, bool isLeftEdge)
        {
            InitializeComponent();
            _trayApp = trayApp;
            _isLeftEdge = isLeftEdge;

            // Setup position
            _screenWidth = SystemParameters.PrimaryScreenWidth;
            _screenHeight = SystemParameters.PrimaryScreenHeight;

            this.Height = _screenHeight;
            this.Width = 1;
            this.Top = 0;
            this.Left = _isLeftEdge ? 0 : _screenWidth - 1;
        }

        private void Window_DragEnter(object sender, DragEventArgs e)
        {
            if (e.Data.GetDataPresent(DataFormats.FileDrop))
            {
                e.Effects = DragDropEffects.Copy;
                ExpandWindow();
            }
            else
            {
                e.Effects = DragDropEffects.None;
            }
            e.Handled = true;
        }

        private void Window_DragLeave(object sender, DragEventArgs e)
        {
            CollapseWindow();
            e.Handled = true;
        }

        private void Window_Drop(object sender, DragEventArgs e)
        {
            if (e.Data.GetDataPresent(DataFormats.FileDrop))
            {
                string[]? files = e.Data.GetData(DataFormats.FileDrop) as string[];
                if (files != null && files.Length > 0)
                {
                    System.Threading.Tasks.Task.Run(() => {
                        _trayApp.PushFilesExternal(files);
                    });
                    NotificationHelper.ShowToast("Deskdrop", $"Sending {files.Length} file(s)...");
                }
            }
            CollapseWindow();
            e.Handled = true;
        }

        private void ExpandWindow()
        {
            // Animate window width
            var widthAnim = new DoubleAnimation(200, TimeSpan.FromMilliseconds(200))
            {
                EasingFunction = new CubicEase() { EasingMode = EasingMode.EaseOut }
            };
            this.BeginAnimation(Window.WidthProperty, widthAnim);

            if (!_isLeftEdge)
            {
                var leftAnim = new DoubleAnimation(_screenWidth - 200, TimeSpan.FromMilliseconds(200))
                {
                    EasingFunction = new CubicEase() { EasingMode = EasingMode.EaseOut }
                };
                this.BeginAnimation(Window.LeftProperty, leftAnim);
            }

            // Animate panel opacity and scale
            var opacityAnim = new DoubleAnimation(1.0, TimeSpan.FromMilliseconds(200));
            PanelBorder.BeginAnimation(UIElement.OpacityProperty, opacityAnim);

            var scaleAnim = new DoubleAnimation(1.0, TimeSpan.FromMilliseconds(200))
            {
                EasingFunction = new BackEase() { Amplitude = 0.5, EasingMode = EasingMode.EaseOut }
            };
            PanelScale.BeginAnimation(ScaleTransform.ScaleXProperty, scaleAnim);
            PanelScale.BeginAnimation(ScaleTransform.ScaleYProperty, scaleAnim);
        }

        private void CollapseWindow()
        {
            // Animate window width back to 1
            var widthAnim = new DoubleAnimation(1, TimeSpan.FromMilliseconds(200))
            {
                EasingFunction = new CubicEase() { EasingMode = EasingMode.EaseIn }
            };
            this.BeginAnimation(Window.WidthProperty, widthAnim);

            if (!_isLeftEdge)
            {
                var leftAnim = new DoubleAnimation(_screenWidth - 1, TimeSpan.FromMilliseconds(200))
                {
                    EasingFunction = new CubicEase() { EasingMode = EasingMode.EaseIn }
                };
                this.BeginAnimation(Window.LeftProperty, leftAnim);
            }

            // Animate panel opacity and scale
            var opacityAnim = new DoubleAnimation(0.0, TimeSpan.FromMilliseconds(200));
            PanelBorder.BeginAnimation(UIElement.OpacityProperty, opacityAnim);

            var scaleAnim = new DoubleAnimation(0.8, TimeSpan.FromMilliseconds(200))
            {
                EasingFunction = new CubicEase() { EasingMode = EasingMode.EaseIn }
            };
            PanelScale.BeginAnimation(ScaleTransform.ScaleXProperty, scaleAnim);
            PanelScale.BeginAnimation(ScaleTransform.ScaleYProperty, scaleAnim);
        }
    }
}
