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
                try { File.AppendAllText(Path.Combine(dir, "winui_trace.txt"), $"[{DateTime.Now:u}] ContextMenu Error: {ex.Message}\n"); } catch (Exception innerEx) { App.HandleError(innerEx); }
            }
        }

        // Registers the deskdrop:// URI scheme under HKCU (no admin needed,
        // same as the context menu above) so Windows actually routes
        // deskdrop://accept/{id}, deskdrop://reject/{id}, and the QR
        // pairing deep link to this app instead of failing to find a
        // handler - none of that worked without this.
        public static void RegisterUriProtocol()
        {
            try
            {
                string exePath = Process.GetCurrentProcess().MainModule?.FileName ?? "";
                if (string.IsNullOrEmpty(exePath)) return;

                using (RegistryKey key = Registry.CurrentUser.CreateSubKey(@"Software\Classes\deskdrop"))
                {
                    key.SetValue("", "URL:Deskdrop Protocol");
                    key.SetValue("URL Protocol", "");
                    using (RegistryKey iconKey = key.CreateSubKey("DefaultIcon"))
                    {
                        iconKey.SetValue("", $"\"{exePath}\",0");
                    }
                    using (RegistryKey shellKey = key.CreateSubKey(@"shell\open\command"))
                    {
                        shellKey.SetValue("", $"\"{exePath}\" \"%1\"");
                    }
                }
            }
            catch (Exception ex)
            {
                var dir = Path.Combine(System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Deskdrop"));
                try { File.AppendAllText(Path.Combine(dir, "winui_trace.txt"), $"[{DateTime.Now:u}] URI Protocol Registration Error: {ex.Message}\n"); } catch (Exception innerEx) { App.HandleError(innerEx); }
            }
        }
    }
}
