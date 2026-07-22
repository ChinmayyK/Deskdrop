using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using System;

namespace Deskdrop.WinUI.Views
{
    public sealed partial class SettingsView : Page
    {
        public DeskdropStore mgr => DeskdropStore.Shared;
        public string DeviceName => Environment.MachineName;

        public SettingsView()
        {
            this.InitializeComponent();
        }

        private void OnStartupToggled(object sender, RoutedEventArgs e)
        {
            // Windows startup registry logic can be invoked here when toggled
        }

        private void OnRescanClicked(object sender, RoutedEventArgs e)
        {
            DaemonClient.RescanPeers();
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
