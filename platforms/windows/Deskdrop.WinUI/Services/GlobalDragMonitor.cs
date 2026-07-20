using System;
using System.Runtime.InteropServices;
using Microsoft.UI.Xaml;
using Microsoft.UI.Dispatching;

namespace Deskdrop.WinUI.Services
{
    public class GlobalDragMonitor : IDisposable
    {
        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool GetCursorPos(out POINT lpPoint);

        [DllImport("user32.dll")]
        private static extern short GetAsyncKeyState(int vKey);

        [DllImport("user32.dll")]
        private static extern int GetSystemMetrics(int nIndex);

        private const int SM_CXSCREEN = 0;
        private const int SM_CYSCREEN = 1;

        [StructLayout(LayoutKind.Sequential)]
        public struct POINT
        {
            public int X;
            public int Y;
        }

        private readonly DispatcherTimer _timer;
        private bool _wasLeftButtonDown = false;
        private UI.EdgeDropWindow? _dropZoneWindow;
        private readonly ClipboardManager _clipboardManager;
        private readonly int _edgeThreshold = 20;

        public GlobalDragMonitor(ClipboardManager clipboardManager)
        {
            _clipboardManager = clipboardManager;
            _timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(100) };
            _timer.Tick += OnTick;
            _timer.Start();
        }

        private void OnTick(object? sender, object e)
        {
            // VK_LBUTTON is 0x01
            bool isLeftButtonDown = (GetAsyncKeyState(0x01) & 0x8000) != 0;

            if (isLeftButtonDown && _wasLeftButtonDown)
            {
                // User is dragging. Are they at the edge?
                if (GetCursorPos(out POINT p))
                {
                    double screenWidth = GetSystemMetrics(SM_CXSCREEN);
                    
                    // If they are on the right edge
                    if (p.X >= screenWidth - _edgeThreshold)
                    {
                        if (_dropZoneWindow == null) // WinUI 3 Window doesn't have IsLoaded
                        {
                            ShowDropZone();
                        }
                    }
                    else if (p.X < screenWidth - 300 - _edgeThreshold) // Hardcoding width since WinUI 3 Window.Bounds is tricky
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
                    App.MainWindow?.DispatcherQueue?.TryEnqueue(HideDropZone);
                });
            }

            _wasLeftButtonDown = isLeftButtonDown;
        }

        private void ShowDropZone()
        {
            if (_dropZoneWindow != null) return;
            
            _dropZoneWindow = new UI.EdgeDropWindow();
            
            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(_dropZoneWindow);
            var windowId = Microsoft.UI.Win32Interop.GetWindowIdFromWindow(hwnd);
            var appWindow = Microsoft.UI.Windowing.AppWindow.GetFromWindowId(windowId);
            
            int screenW = GetSystemMetrics(SM_CXSCREEN);
            int screenH = GetSystemMetrics(SM_CYSCREEN);
            
            // Approximate width/height of the EdgeDropWindow
            int w = 350;
            int h = screenH;
            
            appWindow.MoveAndResize(new Windows.Graphics.RectInt32(screenW - w, 0, w, h));
            
            _dropZoneWindow.Activate();
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



