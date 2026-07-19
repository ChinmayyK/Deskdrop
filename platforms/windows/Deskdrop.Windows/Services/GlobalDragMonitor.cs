using System;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Threading;

namespace Deskdrop.Windows.Services
{
    public class GlobalDragMonitor : IDisposable
    {
        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool GetCursorPos(out POINT lpPoint);

        [DllImport("user32.dll")]
        private static extern short GetAsyncKeyState(int vKey);

        [StructLayout(LayoutKind.Sequential)]
        public struct POINT
        {
            public int X;
            public int Y;
        }

        private readonly DispatcherTimer _timer;
        private bool _wasLeftButtonDown = false;
        private DropZoneWindow? _dropZoneWindow;
        private readonly ClipboardManager _clipboardManager;
        private readonly int _edgeThreshold = 20;

        public GlobalDragMonitor(ClipboardManager clipboardManager)
        {
            _clipboardManager = clipboardManager;
            _timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(100) };
            _timer.Tick += OnTick;
            _timer.Start();
        }

        private void OnTick(object? sender, EventArgs e)
        {
            // VK_LBUTTON is 0x01
            bool isLeftButtonDown = (GetAsyncKeyState(0x01) & 0x8000) != 0;

            if (isLeftButtonDown && _wasLeftButtonDown)
            {
                // User is dragging. Are they at the edge?
                if (GetCursorPos(out POINT p))
                {
                    double screenWidth = SystemParameters.PrimaryScreenWidth;
                    
                    // If they are on the right edge
                    if (p.X >= screenWidth - _edgeThreshold)
                    {
                        if (_dropZoneWindow == null || !_dropZoneWindow.IsLoaded)
                        {
                            ShowDropZone();
                        }
                    }
                    else if (p.X < screenWidth - _dropZoneWindow?.Width - _edgeThreshold)
                    {
                        // If they move away from the window, we shouldn't hide it immediately because they might be trying to drop ON the window.
                        // We will rely on MouseLeave event of DropZoneWindow or letting the Drop handler close it.
                    }
                }
            }
            else if (!isLeftButtonDown && _wasLeftButtonDown)
            {
                // User released the drag. Delay close slightly to allow drop event to process
                System.Threading.Tasks.Task.Delay(500).ContinueWith(_ =>
                {
                    Application.Current?.Dispatcher.Invoke(HideDropZone);
                });
            }

            _wasLeftButtonDown = isLeftButtonDown;
        }

        private void ShowDropZone()
        {
            if (_dropZoneWindow != null) return;
            
            _dropZoneWindow = new DropZoneWindow(_clipboardManager);
            _dropZoneWindow.Left = SystemParameters.PrimaryScreenWidth - _dropZoneWindow.Width;
            _dropZoneWindow.Top = (SystemParameters.PrimaryScreenHeight - _dropZoneWindow.Height) / 2;
            _dropZoneWindow.Show();
        }

        private void HideDropZone()
        {
            if (_dropZoneWindow != null)
            {
                _dropZoneWindow.Close();
                _dropZoneWindow = null;
            }
        }

        public void Dispose()
        {
            _timer.Stop();
            HideDropZone();
        }
    }
}
