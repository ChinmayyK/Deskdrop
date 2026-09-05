using System;
using System.IO;
using Microsoft.UI.Windowing;

namespace Deskdrop.WinUI.Services
{
    public static class WindowIconHelper
    {
        private static readonly string IconPath = Path.Combine(AppContext.BaseDirectory, "Assets", "AppIcon.ico");

        // Unpackaged WinUI3 apps don't automatically pick up the exe's
        // embedded icon for window/titlebar/taskbar - it must be set
        // explicitly per AppWindow, otherwise it falls back to a generic
        // WinUI icon.
        public static void Apply(AppWindow appWindow)
        {
            try
            {
                if (File.Exists(IconPath)) appWindow.SetIcon(IconPath);
            }
            catch (Exception ex) { App.HandleError(ex); }
        }

        [System.Runtime.InteropServices.DllImport("user32.dll")]
        private static extern uint GetDpiForWindow(IntPtr hWnd);

        // AppWindow.Resize/Move take physical pixels, but every size in this
        // app (fonts, paddings, and every hardcoded window width/height) is
        // authored in DIPs. Without this, a window is only the intended size
        // on a 100%-scaled display; on anything scaled higher its physical
        // pixel dimensions are smaller in DIP terms than the layout assumes,
        // so text/buttons/icons all read as oversized for the space they're
        // crammed into.
        public static double GetDpiScale(IntPtr hwnd)
        {
            try { return GetDpiForWindow(hwnd) / 96.0; }
            catch { return 1.0; }
        }

        public static void ResizeDips(AppWindow appWindow, IntPtr hwnd, int widthDips, int heightDips)
        {
            double scale = GetDpiScale(hwnd);
            appWindow.Resize(new Windows.Graphics.SizeInt32((int)(widthDips * scale), (int)(heightDips * scale)));
        }
    }
}
