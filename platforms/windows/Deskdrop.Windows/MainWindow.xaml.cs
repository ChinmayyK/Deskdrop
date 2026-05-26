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
        private bool _hasCompletedOnboarding;

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
            Hide();
        }

        protected override void OnClosing(System.ComponentModel.CancelEventArgs e)
        {
            // Instead of closing the application, we just hide the window to keep it running in the tray.
            e.Cancel = true;
            Hide();
        }

        private void NavDevices_Click(object sender, RoutedEventArgs e)
        {
            LoadDevicesView();
        }

        private void NavSettings_Click(object sender, RoutedEventArgs e)
        {
            LoadSettingsView();
        }

        private void BtnBack_Click(object sender, RoutedEventArgs e)
        {
            LoadHomeView();
        }

        public void ShowToast(string message, bool isError = false)
        {
            Dispatcher.Invoke(() =>
            {
                NotificationMessage.Text = message;
                if (isError)
                {
                    NotificationIcon.Text = "\xE783"; // Warning icon
                    NotificationIcon.Foreground = new SolidColorBrush(Color.FromRgb(255, 59, 48)); // Red
                }
                else
                {
                    NotificationIcon.Text = "\xE946"; // Info icon
                    NotificationIcon.Foreground = (SolidColorBrush)FindResource("BrandElectric");
                }
                
                NotificationToast.Visibility = Visibility.Visible;
                
                // Auto-hide after 3 seconds
                System.Threading.Tasks.Task.Delay(3000).ContinueWith(_ => 
                {
                    Dispatcher.Invoke(() => NotificationToast.Visibility = Visibility.Collapsed);
                });
            });
        }

        private void LoadHomeView()
        {
            if (HomeView != null) HomeView.Visibility = Visibility.Visible;
            if (DevicesView != null) DevicesView.Visibility = Visibility.Collapsed;
            if (SettingsView != null) SettingsView.Visibility = Visibility.Collapsed;
            
            _hasCompletedOnboarding = Program.LoadSettings().HasCompletedOnboarding;
            UpdateOnboardingVisibility();
            
            if (TimelineList != null)
                TimelineList.ItemsSource = _clipboardManager.GetHistory();
        }

        private void UpdateOnboardingVisibility()
        {
            if (_hasCompletedOnboarding)
            {
                if (OnboardingGrid != null) OnboardingGrid.Visibility = Visibility.Collapsed;
                if (QuickActionsRibbon != null) QuickActionsRibbon.Visibility = Visibility.Visible;
            }
            else
            {
                if (OnboardingGrid != null) OnboardingGrid.Visibility = Visibility.Visible;
                if (QuickActionsRibbon != null) QuickActionsRibbon.Visibility = Visibility.Collapsed;
            }
        }
        
        private void BtnDismissOnboarding_Click(object sender, RoutedEventArgs e)
        {
            _hasCompletedOnboarding = true;
            Program.CompleteOnboarding();
            UpdateOnboardingVisibility();
        }

        private void LoadDevicesView()
        {
            if (HomeView != null) HomeView.Visibility = Visibility.Collapsed;
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
                            if (!_hasCompletedOnboarding)
                            {
                                UpdateOnboardingStatus(peers);
                            }
                        });
                    }
                }
                catch { /* ignored */ }
            });
        }

        private void UpdateOnboardingStatus(System.Collections.Generic.List<PeerViewModel> peers)
        {
            if (_hasCompletedOnboarding) return;
            
            bool foundDevice = peers.Count > 0;
            bool pairedDevice = peers.Exists(p => p.Trusted);
            
            if (Step1Icon != null)
            {
                Step1Icon.Text = foundDevice ? "\uE73E" : "\uE73E"; // Same icon but we can change color
                Step1Icon.Foreground = new SolidColorBrush(foundDevice ? (Color)ColorConverter.ConvertFromString("#32ADE6") : (Color)ColorConverter.ConvertFromString("#38383A"));
                Step1Text.Foreground = new SolidColorBrush(foundDevice ? (Color)ColorConverter.ConvertFromString("#FFFFFF") : (Color)ColorConverter.ConvertFromString("#8E8E93"));
            }
            
            if (Step2Icon != null)
            {
                Step2Icon.Foreground = new SolidColorBrush(pairedDevice ? (Color)ColorConverter.ConvertFromString("#32ADE6") : (Color)ColorConverter.ConvertFromString("#38383A"));
                Step2Text.Foreground = new SolidColorBrush(pairedDevice ? (Color)ColorConverter.ConvertFromString("#FFFFFF") : (Color)ColorConverter.ConvertFromString("#8E8E93"));
            }

            if (Step3Icon != null)
            {
                Step3Icon.Foreground = new SolidColorBrush(pairedDevice ? (Color)ColorConverter.ConvertFromString("#32ADE6") : (Color)ColorConverter.ConvertFromString("#38383A"));
                Step3Text.Foreground = new SolidColorBrush(pairedDevice ? (Color)ColorConverter.ConvertFromString("#FFFFFF") : (Color)ColorConverter.ConvertFromString("#8E8E93"));
            }

            if (pairedDevice && BtnDismissOnboarding != null)
            {
                BtnDismissOnboarding.Visibility = Visibility.Visible;
            }
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
            
            ShowToast("Settings saved and applied.");
        }

        private void BorderPushClipboard_Click(object sender, MouseButtonEventArgs e)
        {
            System.Threading.Tasks.Task.Run(() =>
            {
                bool ok = DaemonClient.Send(new { cmd = "push_clipboard" }) != null;
                if (ok)
                {
                    ShowToast("Clipboard sent to connected devices.");
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
                ShowToast($"Sending file: {System.IO.Path.GetFileName(file)}");
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
                        ShowToast("Could not reach daemon. Make sure Deskdrop is running.", true);
                    }
                    else
                    {
                        ShowToast($"Connecting to {addr}...");
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
                BorderBroadcastCamera.Background = new SolidColorBrush(System.Windows.Media.Color.FromArgb(0xFF, 0xFF, 0xEB, 0xEC)); // #FFEBEC
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
                    BorderBroadcastCamera.Background = new SolidColorBrush(System.Windows.Media.Color.FromArgb(0xFF, 0xFF, 0xCD, 0xD2)); // Stronger red tint
                }
                catch (Exception ex)
                {
                    _isBroadcasting = false;
                    _cameraPublisher?.Dispose();
                    _cameraPublisher = null;
                    
                    TxtBroadcastTitle.Text = "Broadcast Camera";
                    ShowToast($"Failed to start camera: {ex.Message}", true);
                }
            }
        }

        private void BtnShowQR_Click(object sender, RoutedEventArgs e)
        {
            var qrWindow = new QRPairingWindow();
            qrWindow.Owner = this;
            qrWindow.ShowDialog();
        }

        private string? _currentTofuDevice;

        public void ShowTofuPrompt(string deviceName, string fingerprint)
        {
            Dispatcher.Invoke(() =>
            {
                _currentTofuDevice = deviceName;
                TxtTofuDeviceName.Text = deviceName;
                TxtTofuFingerprint.Text = FormatFingerprint(fingerprint);
                TofuPromptOverlay.Visibility = Visibility.Visible;
            });
        }

        private void BtnTofuTrust_Click(object sender, RoutedEventArgs e)
        {
            if (_currentTofuDevice != null)
            {
                _clipboardManager?.RespondToTrust(_currentTofuDevice, true);
                ShowToast($"{_currentTofuDevice} trusted.");
                _currentTofuDevice = null;
            }
            TofuPromptOverlay.Visibility = Visibility.Collapsed;
        }

        private void BtnTofuReject_Click(object sender, RoutedEventArgs e)
        {
            if (_currentTofuDevice != null)
            {
                _clipboardManager?.RespondToTrust(_currentTofuDevice, false);
                _currentTofuDevice = null;
            }
            TofuPromptOverlay.Visibility = Visibility.Collapsed;
        }

        private static string FormatFingerprint(string raw)
        {
            var clean = raw.Replace(":", "").ToUpperInvariant();
            var pairs = new System.Collections.Generic.List<string>();
            for (int i = 0; i + 1 < clean.Length; i += 2)
                pairs.Add(clean.Substring(i, 2));
            var lines = new System.Collections.Generic.List<string>();
            for (int i = 0; i < pairs.Count; i += 8)
                lines.Add(string.Join(":", pairs.GetRange(i, Math.Min(8, pairs.Count - i))));
            return string.Join("\n", lines);
        }
    }
}
