using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;

namespace Deskdrop.Windows
{
    public partial class MainWindow : Window
    {
        private readonly ClipboardManager _clipboardManager;
        private CameraPublisher? _cameraPublisher;
        private bool _isBroadcasting;

        public MainWindow(ClipboardManager clipboardManager)
        {
            InitializeComponent();
            _clipboardManager = clipboardManager;
            _clipboardManager.HistoryItemAdded += OnHistoryItemAdded;
            LoadHomeView();
        }

        private void OnHistoryItemAdded(HistoryItem obj)
        {
            Dispatcher.Invoke(() =>
            {
                if (TimelineView != null && TimelineView.Visibility == Visibility.Visible)
                {
                    TimelineList.ItemsSource = _clipboardManager.GetHistory();
                }
            });
        }

        protected override void OnMouseLeftButtonDown(MouseButtonEventArgs e)
        {
            base.OnMouseLeftButtonDown(e);
            DragMove();
        }

        private void BtnMinimize_Click(object sender, RoutedEventArgs e)
        {
            WindowState = WindowState.Minimized;
        }

        private void BtnClose_Click(object sender, RoutedEventArgs e)
        {
            // Instead of closing the application, we just hide the window to keep it running in the tray.
            Hide();
        }

        private void NavHome_Checked(object sender, RoutedEventArgs e)
        {
            LoadHomeView();
        }

        private void NavTimeline_Checked(object sender, RoutedEventArgs e)
        {
            LoadTimelineView();
        }

        private void NavDevices_Checked(object sender, RoutedEventArgs e)
        {
            LoadDevicesView();
        }

        private void NavSettings_Checked(object sender, RoutedEventArgs e)
        {
            LoadSettingsView();
        }

        private void LoadHomeView()
        {
            if (HomeView != null) HomeView.Visibility = Visibility.Visible;
            if (TimelineView != null) TimelineView.Visibility = Visibility.Collapsed;
            if (DevicesView != null) DevicesView.Visibility = Visibility.Collapsed;
            if (SettingsView != null) SettingsView.Visibility = Visibility.Collapsed;
        }

        private void LoadTimelineView()
        {
            if (HomeView != null) HomeView.Visibility = Visibility.Collapsed;
            if (TimelineView != null) TimelineView.Visibility = Visibility.Visible;
            if (DevicesView != null) DevicesView.Visibility = Visibility.Collapsed;
            if (SettingsView != null) SettingsView.Visibility = Visibility.Collapsed;
            
            // Reload timeline items
            if (_clipboardManager != null && TimelineList != null)
            {
                TimelineList.ItemsSource = _clipboardManager.GetHistory();
            }
        }

        private void LoadDevicesView()
        {
            if (HomeView != null) HomeView.Visibility = Visibility.Collapsed;
            if (TimelineView != null) TimelineView.Visibility = Visibility.Collapsed;
            if (DevicesView != null) DevicesView.Visibility = Visibility.Visible;
            if (SettingsView != null) SettingsView.Visibility = Visibility.Collapsed;
            
            RefreshDevicesList();
        }

        private void RefreshDevicesList()
        {
            System.Threading.Tasks.Task.Run(() =>
            {
                try
                {
                    var state = DaemonClient.Status();
                    if (state != null && state.RootElement.TryGetProperty("peers", out var peersElem))
                    {
                        var peers = System.Text.Json.JsonSerializer.Deserialize<System.Collections.Generic.List<PeerViewModel>>(peersElem.GetRawText());
                        Dispatcher.Invoke(() =>
                        {
                            if (DevicesList != null) DevicesList.ItemsSource = peers;
                        });
                    }
                }
                catch { /* ignored */ }
            });
        }

        private void BtnDisconnectDevice_Click(object sender, RoutedEventArgs e)
        {
            if (sender is System.Windows.Controls.Button btn && btn.Tag is string deviceId)
            {
                System.Threading.Tasks.Task.Run(() =>
                {
                    DaemonClient.Send(new { cmd = "disconnect_peer", device_id = deviceId });
                    RefreshDevicesList();
                });
            }
        }
        
        public class PeerViewModel
        {
            public string device_id { get; set; } = "";
            public string friendly_name { get; set; } = "";
            public string status { get; set; } = "";
        }

        private void LoadSettingsView()
        {
            if (HomeView != null) HomeView.Visibility = Visibility.Collapsed;
            if (TimelineView != null) TimelineView.Visibility = Visibility.Collapsed;
            if (DevicesView != null) DevicesView.Visibility = Visibility.Collapsed;
            if (SettingsView != null) SettingsView.Visibility = Visibility.Visible;
            
            LoadSettings();
        }

        private void LoadSettings()
        {
            using var key = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(@"Software\Deskdrop");
            if (key == null) return;

            ChkSyncEnabled.IsChecked = (int?)key.GetValue("SyncEnabled", 1) == 1;
            ChkSyncText.IsChecked = (int?)key.GetValue("SyncText", 1) == 1;
            ChkSyncImages.IsChecked = (int?)key.GetValue("SyncImages", 1) == 1;
            ChkSyncFiles.IsChecked = (int?)key.GetValue("SyncFiles", 1) == 1;
            ChkShowNotifications.IsChecked = (int?)key.GetValue("ShowNotifications", 1) == 1;
            ChkRequireTofu.IsChecked = (int?)key.GetValue("RequireTofu", 1) == 1;
            TxtDeviceName.Text = (string?)key.GetValue("DeviceName", "") ?? "";
        }

        private void BtnSaveSettings_Click(object sender, RoutedEventArgs e)
        {
            using var key = Microsoft.Win32.Registry.CurrentUser.CreateSubKey(@"Software\Deskdrop");
            key.SetValue("SyncEnabled", ChkSyncEnabled.IsChecked == true ? 1 : 0, Microsoft.Win32.RegistryValueKind.DWord);
            key.SetValue("SyncText", ChkSyncText.IsChecked == true ? 1 : 0, Microsoft.Win32.RegistryValueKind.DWord);
            key.SetValue("SyncImages", ChkSyncImages.IsChecked == true ? 1 : 0, Microsoft.Win32.RegistryValueKind.DWord);
            key.SetValue("SyncFiles", ChkSyncFiles.IsChecked == true ? 1 : 0, Microsoft.Win32.RegistryValueKind.DWord);
            key.SetValue("ShowNotifications", ChkShowNotifications.IsChecked == true ? 1 : 0, Microsoft.Win32.RegistryValueKind.DWord);
            key.SetValue("RequireTofu", ChkRequireTofu.IsChecked == true ? 1 : 0, Microsoft.Win32.RegistryValueKind.DWord);
            key.SetValue("DeviceName", TxtDeviceName.Text, Microsoft.Win32.RegistryValueKind.String);

            // Trigger update in daemon
            System.Threading.Tasks.Task.Run(() =>
            {
                DaemonClient.Send(new
                {
                    cmd = "save_settings",
                    sync_enabled = ChkSyncEnabled.IsChecked == true,
                    sync_text = ChkSyncText.IsChecked == true,
                    sync_images = ChkSyncImages.IsChecked == true,
                    sync_files = ChkSyncFiles.IsChecked == true,
                    device_name = string.IsNullOrWhiteSpace(TxtDeviceName.Text) ? null : TxtDeviceName.Text,
                    require_tofu_confirmation = ChkRequireTofu.IsChecked == true,
                    show_receive_notification = ChkShowNotifications.IsChecked == true,
                });
            });
            
            System.Windows.MessageBox.Show("Settings saved and applied.", "Deskdrop", MessageBoxButton.OK, MessageBoxImage.Information);
        }

        private void BorderPushClipboard_Click(object sender, MouseButtonEventArgs e)
        {
            System.Threading.Tasks.Task.Run(() =>
            {
                bool ok = DaemonClient.Send(new { cmd = "push_clipboard" }) != null;
                if (ok)
                {
                    Dispatcher.Invoke(() => System.Windows.MessageBox.Show("Clipboard sent to connected devices.", "Deskdrop", MessageBoxButton.OK, MessageBoxImage.Information));
                }
            });
        }

        private void BorderSendFiles_Click(object sender, MouseButtonEventArgs e)
        {
            var dlg = new Microsoft.Win32.OpenFileDialog();
            dlg.Multiselect = false;
            dlg.Title = "Select File to Send";
            if (dlg.ShowDialog() == true)
            {
                var file = dlg.FileName;
                _clipboardManager?.PushFile(file);
                System.Windows.MessageBox.Show($"Sending file: {System.IO.Path.GetFileName(file)}", "Deskdrop", MessageBoxButton.OK, MessageBoxImage.Information);
            }
        }

        private void BorderStreamCamera_Click(object sender, MouseButtonEventArgs e)
        {
            var previewWindow = new CameraPreviewWindow();
            previewWindow.Show();
        }

        private void BtnConnect_Click(object sender, RoutedEventArgs e)
        {
            var addr = TxtConnectAddress.Text?.Trim();
            if (string.IsNullOrEmpty(addr)) return;

            System.Threading.Tasks.Task.Run(() =>
            {
                var parts = addr.Split(':', 2, StringSplitOptions.RemoveEmptyEntries);
                object cmd = parts.Length == 2 && ushort.TryParse(parts[1], out var p)
                    ? new { cmd = "connect_manual", host = parts[0], port = (int)p }
                    : (object)new { cmd = "connect_manual", host = addr };
                
                var resp = DaemonClient.Send(cmd);
                Dispatcher.Invoke(() =>
                {
                    if (resp == null)
                    {
                        System.Windows.MessageBox.Show("Could not reach daemon.\nMake sure Deskdrop is running.", "Connection failed", MessageBoxButton.OK, MessageBoxImage.Warning);
                    }
                    else
                    {
                        System.Windows.MessageBox.Show($"Connecting to {addr}...", "Deskdrop", MessageBoxButton.OK, MessageBoxImage.Information);
                    }
                });
            });
        }

        private async void BorderBroadcastCamera_Click(object sender, MouseButtonEventArgs e)
        {
            if (_isBroadcasting)
            {
                _isBroadcasting = false;
                if (_cameraPublisher != null)
                {
                    _cameraPublisher.StopBroadcasting();
                    _cameraPublisher.Dispose();
                    _cameraPublisher = null;
                }
                
                TxtBroadcastTitle.Text = "Broadcast Camera";
                TxtBroadcastDesc.Text = "Push local webcam to peers";
                BorderBroadcastCamera.Background = new SolidColorBrush(System.Windows.Media.Color.FromArgb(0x11, 0xFF, 0x3B, 0x30)); // Red tint
            }
            else
            {
                try
                {
                    _isBroadcasting = true;
                    TxtBroadcastTitle.Text = "Starting...";
                    
                    _cameraPublisher = new CameraPublisher(_clipboardManager);
                    await _cameraPublisher.StartBroadcastingAsync();
                    
                    TxtBroadcastTitle.Text = "Stop Broadcasting";
                    TxtBroadcastDesc.Text = "Camera is live (Broadcasting)";
                    BorderBroadcastCamera.Background = new SolidColorBrush(System.Windows.Media.Color.FromArgb(0x33, 0xFF, 0x3B, 0x30)); // Stronger red tint
                }
                catch (Exception ex)
                {
                    _isBroadcasting = false;
                    _cameraPublisher?.Dispose();
                    _cameraPublisher = null;
                    
                    TxtBroadcastTitle.Text = "Broadcast Camera";
                    System.Windows.MessageBox.Show($"Failed to start camera: {ex.Message}", "Deskdrop", MessageBoxButton.OK, MessageBoxImage.Error);
                }
            }
        }

        private void BtnShowQR_Click(object sender, RoutedEventArgs e)
        {
            var qrWindow = new QRPairingWindow();
            qrWindow.Owner = this;
            qrWindow.ShowDialog();
        }
    }
}
