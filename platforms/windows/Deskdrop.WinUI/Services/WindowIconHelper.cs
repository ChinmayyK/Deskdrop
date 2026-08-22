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
    }
}
