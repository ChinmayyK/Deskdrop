using Microsoft.Win32;
using System;
using System.Diagnostics;
using System.IO;

namespace Deskdrop.WinUI.Native
{
    public static class ContextMenuIntegration
    {
        public static void RegisterContextMenu()
        {
            try
            {
                string exePath = Process.GetCurrentProcess().MainModule?.FileName ?? "";
                if (string.IsNullOrEmpty(exePath)) return;

                // Register for all files
                using (RegistryKey key = Registry.CurrentUser.CreateSubKey(@"Software\Classes\*\shell\Deskdrop"))
                {
                    key.SetValue("", "Send via Deskdrop");
                    key.SetValue("Icon", $"\"{exePath}\",0");
                    using (RegistryKey commandKey = key.CreateSubKey("command"))
                    {
                        commandKey.SetValue("", $"\"{exePath}\" \"%1\"");
                    }
                }

                // Register for directories
                using (RegistryKey key = Registry.CurrentUser.CreateSubKey(@"Software\Classes\Directory\shell\Deskdrop"))
                {
                    key.SetValue("", "Send via Deskdrop");
                    key.SetValue("Icon", $"\"{exePath}\",0");
                    using (RegistryKey commandKey = key.CreateSubKey("command"))
                    {
                        commandKey.SetValue("", $"\"{exePath}\" \"%1\"");
                    }
                }
            }
            catch (Exception ex)
            {
                var dir = Path.Combine(System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Deskdrop"));
                try { File.AppendAllText(Path.Combine(dir, "winui_trace.txt"), $"[{DateTime.Now:u}] ContextMenu Error: {ex.Message}\n"); } catch (Exception ex) { System.Diagnostics.Debug.WriteLine($"Swallowed Exception: {ex.Message}\n{ex.StackTrace}"); }
            }
        }
    }
}
