using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using System;

namespace Deskdrop.WinUI.Views
{
    public sealed partial class SettingsView : Page
    {
        public DeskdropStore mgr => DeskdropStore.Shared;
        public string DeviceName => Environment.MachineName;

        private const string StartupRegistryKeyPath = @"Software\Microsoft\Windows\CurrentVersion\Run";
        private const string StartupRegistryValueName = "Deskdrop";

        public SettingsView()
        {
            this.InitializeComponent();
            StartupToggle.IsOn = IsLaunchAtStartupEnabled();
        }

        private static bool IsLaunchAtStartupEnabled()
        {
            try
            {
                using var key = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(StartupRegistryKeyPath, writable: false);
                return key?.GetValue(StartupRegistryValueName) != null;
            }
            catch (Exception ex)
            {
                App.HandleError(ex);
                return false;
            }
        }

        private void OnStartupToggled(object sender, RoutedEventArgs e)
        {
            try
            {
                using var key = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(StartupRegistryKeyPath, writable: true);
                if (key == null) return;

                if (StartupToggle.IsOn)
                {
                    var exePath = Environment.ProcessPath ?? System.Reflection.Assembly.GetExecutingAssembly().Location;
                    key.SetValue(StartupRegistryValueName, $"\"{exePath}\"");
                }
                else
                {
                    key.DeleteValue(StartupRegistryValueName, throwOnMissingValue: false);
                }
            }
            catch (Exception ex)
            {
                App.HandleError(ex);
            }
        }

        private void OnRescanClicked(object sender, RoutedEventArgs e)
        {
            DaemonClient.RescanPeers();
        }

        private void OnOpenDownloadsClicked(object sender, RoutedEventArgs e)
        {
            try
            {
                var downloadsPath = System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), "Downloads");
                System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo
                {
                    FileName = downloadsPath,
                    UseShellExecute = true
                });
            }
            catch (Exception ex)
            {
                App.HandleError(ex);
            }
        }

        private void OnRevokeAllClicked(object sender, RoutedEventArgs e)
        {
            try
            {
                foreach (var peer in mgr.Peers)
                {
                    mgr.ForgetPeer(peer.device_id);
                }
            }
            catch (Exception ex)
            {
                App.HandleError(ex);
            }
        }

        private void OnViewLogsClicked(object sender, RoutedEventArgs e)
        {
            try
            {
                var dir = System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Deskdrop");
                System.IO.Directory.CreateDirectory(dir);
                System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo
                {
                    FileName = dir,
                    UseShellExecute = true
                });
            }
            catch (Exception ex)
            {
                App.HandleError(ex);
            }
        }

        private void OnRevertClicked(object sender, RoutedEventArgs e)
        {
            mgr.UpdateStateFromDaemon();
        }

        private void OnSaveClicked(object sender, RoutedEventArgs e)
        {
            DaemonClient.PatchSettings(new { sync_enabled = mgr.SyncEnabled });
        }
    }
}
