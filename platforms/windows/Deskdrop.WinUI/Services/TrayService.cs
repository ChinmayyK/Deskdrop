using System;
using Microsoft.UI.Xaml;
using Deskdrop.TrayHelper;

namespace Deskdrop.WinUI.Services
{
    public class TrayService : IDisposable
    {
        private readonly TrayManager? _manager;

        public static TrayService? Current { get; private set; }

        public TrayService()
        {
            Current = this;
            try
            {
                _manager = new TrayManager();
                _manager.OpenRequested += OnOpenRequested;
                _manager.SettingsRequested += OnSettingsRequested;
                _manager.RescanRequested += OnRescanRequested;
                _manager.ExitRequested += OnExitRequested;
            }
            catch (Exception ex)
            {
                App.HandleError(ex);
            }
        }

        public TrayService(IntPtr unusedHwnd) : this()
        {
        }

        private void OnOpenRequested()
        {
            App.MainDispatcherQueue?.TryEnqueue(() =>
            {
                ((App)Application.Current).ShowMainWindowCommand?.Execute(null);
            });
        }

        private void OnSettingsRequested()
        {
            App.MainDispatcherQueue?.TryEnqueue(() =>
            {
                ((App)Application.Current).ShowMainWindowCommand?.Execute(null);
                DashboardWindow.Current?.NavigateTo("Settings");
            });
        }

        private void OnRescanRequested()
        {
            DaemonClient.RescanPeers();
        }

        private void OnExitRequested()
        {
            App.MainDispatcherQueue?.TryEnqueue(() =>
            {
                ((App)Application.Current).ExitApplicationCommand?.Execute(null);
            });
        }

        public void UpdateTooltip(string text)
        {
            _manager?.UpdateTooltip(text);
        }

        public void ShowNotification(string title, string text)
        {
            _manager?.ShowNotification(title, text);
        }

        public void Dispose()
        {
            _manager?.Dispose();
        }
    }
}
