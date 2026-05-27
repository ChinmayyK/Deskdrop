using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Linq;

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
            _clipboardManager.QuickContextUpdated += OnQuickContextUpdated;
            LoadHomeView();
        }

        [System.Runtime.InteropServices.DllImport("dwmapi.dll")]
        public static extern int DwmSetWindowAttribute(IntPtr hwnd, int dwAttribute, ref int pvAttribute, int cbAttribute);
        const int DWMWA_USE_MICA = 1029; 
        const int DWMWA_USE_IMMERSIVE_DARK_MODE = 20;

        protected override void OnSourceInitialized(EventArgs e)
        {
            base.OnSourceInitialized(e);
            try {
                IntPtr hwnd = new System.Windows.Interop.WindowInteropHelper(this).Handle;
                int trueValue = 1;
                DwmSetWindowAttribute(hwnd, DWMWA_USE_MICA, ref trueValue, System.Runtime.InteropServices.Marshal.SizeOf(trueValue));
                DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, ref trueValue, System.Runtime.InteropServices.Marshal.SizeOf(trueValue));
            } catch { /* Ignore on older OS */ }
        }

        private void AnimateView(FrameworkElement view)
        {
            if (view == null) return;
            view.Visibility = Visibility.Visible;
            if (FindResource("FadeInTransition") is System.Windows.Media.Animation.Storyboard sb)
            {
                sb.Begin(view);
            }
        }

        private void LoadTransfersView()
        {
            HideAllViews();
            if (TransfersView != null) AnimateView(TransfersView);
            
            // Populate history
            if (TransfersHistoryList != null)
            {
                TransfersHistoryList.ItemsSource = _clipboardManager.GetHistory()
                    .Where(h => h.TypeIcon == "📎" || h.Summary.Contains("File"))
                    .ToList();
            }
        }

        private void HideAllViews()
        {
            if (HomeView != null) HomeView.Visibility = Visibility.Collapsed;
            if (DevicesView != null) DevicesView.Visibility = Visibility.Collapsed;
            if (SettingsView != null) SettingsView.Visibility = Visibility.Collapsed;
            if (DiagnosticsView != null) DiagnosticsView.Visibility = Visibility.Collapsed;
            if (TransfersView != null) TransfersView.Visibility = Visibility.Collapsed;
        }

        private void OnQuickContextUpdated(string? text)
        {
            Dispatcher.Invoke(() =>
            {
                if (string.IsNullOrEmpty(text))
                {
                    QuickContextStrip.Visibility = Visibility.Collapsed;
                }
                else
                {
                    TxtQuickContext.Text = text;
                    QuickContextStrip.Visibility = Visibility.Visible;
                }
            });
        }

        private void OnHistoryItemAdded(HistoryItem obj)
        {
            Dispatcher.Invoke(() =>
            {
                if (HomeView != null && HomeView.Visibility == Visibility.Visible)
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

        private void NavTransfers_Click(object sender, RoutedEventArgs e)
        {
            LoadTransfersView();
        }

        private void NavSettings_Click(object sender, RoutedEventArgs e)
        {
            LoadSettingsView();
        }

        private void NavDiagnostics_Click(object sender, RoutedEventArgs e)
        {
            LoadDiagnosticsView();
        }

        private void LoadDiagnosticsView()
        {
            HideAllViews();
            if (DiagnosticsView != null) AnimateView(DiagnosticsView);
            
            RefreshDiagnosticsState();
        }

        private void RefreshDiagnosticsState()
        {
            System.Threading.Tasks.Task.Run(() =>
            {
                bool isRunning = DaemonClient.IsDaemonRunning();
                int peerCount = 0;

                if (isRunning)
                {
                    try
                    {
                        var state = DaemonClient.Status();
                        if (state != null && state.RootElement.TryGetProperty("data", out var data) && data.TryGetProperty("peer_count", out var pc))
                        {
                            peerCount = pc.GetInt32();
                        }
                    }
                    catch { /* ignore */ }
                }

                Dispatcher.Invoke(() =>
                {
                    if (TxtDiagDaemonStatus != null)
                    {
                        TxtDiagDaemonStatus.Text = isRunning ? "Running" : "Stopped";
                        TxtDiagDaemonSuggestion.Visibility = isRunning ? Visibility.Collapsed : Visibility.Visible;
                        BtnRestartConnection.Visibility = isRunning ? Visibility.Collapsed : Visibility.Visible;
                    }

                    if (TxtDiagNetworkStatus != null)
                    {
                        TxtDiagNetworkStatus.Text = peerCount > 0 ? $"Connected to {peerCount} peers" : "Looking for peers";
                        TxtDiagNetworkSuggestion.Visibility = peerCount > 0 ? Visibility.Collapsed : Visibility.Visible;
                        BtnScanAgain.Visibility = peerCount > 0 ? Visibility.Collapsed : Visibility.Visible;
                    }
                });
            });
        }

        private void BtnRestartConnection_Click(object sender, RoutedEventArgs e)
        {
            System.Diagnostics.Process.Start(System.Reflection.Assembly.GetExecutingAssembly().Location);
            System.Windows.Application.Current.Shutdown();
        }

        private void BtnScanAgain_Click(object sender, RoutedEventArgs e)
        {
            DaemonClient.Send(new { cmd = "rescan_peers" });
            RefreshDiagnosticsState();
        }

        private void BtnPinItem_Click(object sender, RoutedEventArgs e)
        {
            if (sender is System.Windows.Controls.Button btn && btn.DataContext is HistoryItem item)
            {
                _clipboardManager.TogglePinHistory(item.Id);
                TimelineList.ItemsSource = _clipboardManager.GetHistory();
            }
        }

        private void BtnSendQuickContext_Click(object sender, RoutedEventArgs e)
        {
            if (!string.IsNullOrEmpty(_clipboardManager.QuickContextText))
            {
                System.Threading.Tasks.Task.Run(() =>
                {
                    DaemonClient.Send(new { cmd = "push_clipboard" });
                });
                
                // Hide the strip after sending
                QuickContextStrip.Visibility = Visibility.Collapsed;
                ShowToast("Pushed to devices");
            }
        }

        private void BtnDeleteItem_Click(object sender, RoutedEventArgs e)
        {
            if (sender is System.Windows.Controls.Button btn && btn.DataContext is HistoryItem item)
            {
                _clipboardManager.DeleteHistory(item.Id);
                TimelineList.ItemsSource = _clipboardManager.GetHistory();
            }
        }

        private void BtnBack_Click(object sender, RoutedEventArgs e)
        {
            LoadHomeView();
        }

        private void NavHome_Click(object sender, RoutedEventArgs e)
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
                    NotificationIcon.Foreground = new SolidColorBrush(System.Windows.Media.Color.FromRgb(255, 59, 48)); // Red
                }
                else
                {
                    NotificationIcon.Text = "\xE946"; // Info icon
                    NotificationIcon.Foreground = (SolidColorBrush)FindResource("BrandElectric");
                }
                
                NotificationToast.Visibility = Visibility.Visible;
                if (FindResource("ToastSlideIn") is System.Windows.Media.Animation.Storyboard slideIn)
                {
                    slideIn.Begin(NotificationToast);
                }
                
                // Auto-hide after 3 seconds
                System.Threading.Tasks.Task.Delay(3000).ContinueWith(_ => 
                {
                    Dispatcher.Invoke(() => 
                    {
                        if (FindResource("ToastSlideOut") is System.Windows.Media.Animation.Storyboard slideOut)
                        {
                            slideOut.Completed += (s, ev) => NotificationToast.Visibility = Visibility.Collapsed;
                            slideOut.Begin(NotificationToast);
                        }
                        else
                        {
                            NotificationToast.Visibility = Visibility.Collapsed;
                        }
                    });
                });
            });
        }

        private void LoadHomeView()
        {
            HideAllViews();
            if (HomeView != null) AnimateView(HomeView);
            
            _hasCompletedOnboarding = TrayApp.LoadSettings().HasCompletedOnboarding;
            UpdateOnboardingVisibility();
            
            if (TimelineList != null)
                TimelineList.ItemsSource = _clipboardManager.GetHistory();
                
            if (!_hasCompletedOnboarding)
            {
                RefreshDevicesList();
            }
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
            TrayApp.CompleteOnboarding();
            UpdateOnboardingVisibility();
        }

        private void LoadDevicesView()
        {
            HideAllViews();
            if (DevicesView != null) AnimateView(DevicesView);
            
            RefreshDevicesList();
        }

        private void RefreshDevicesList()
        {
            System.Threading.Tasks.Task.Run(() =>
            {
                try
                {
                    var state = DaemonClient.Status();
                    if (state != null && state.RootElement.TryGetProperty("data", out var dataElem))
                    {
                        if (dataElem.TryGetProperty("peers", out var peersElem))
                        {
                            var peers = System.Text.Json.JsonSerializer.Deserialize<System.Collections.Generic.List<PeerViewModel>>(peersElem.GetRawText());
                            Dispatcher.Invoke(() =>
                            {
                                if (DevicesList != null) DevicesList.ItemsSource = peers;
                                if (!_hasCompletedOnboarding && peers != null)
                                {
                                    UpdateOnboardingStatus(peers);
                                }
                            });
                        }
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
                Step1Icon.Foreground = new SolidColorBrush(foundDevice ? (System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#32ADE6") : (System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#38383A"));
                Step1Text.Foreground = new SolidColorBrush(foundDevice ? (System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#FFFFFF") : (System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#8E8E93"));
            }
            
            if (Step2Icon != null)
            {
                Step2Icon.Foreground = new SolidColorBrush(pairedDevice ? (System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#32ADE6") : (System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#38383A"));
                Step2Text.Foreground = new SolidColorBrush(pairedDevice ? (System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#FFFFFF") : (System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#8E8E93"));
            }

            if (Step3Icon != null)
            {
                Step3Icon.Foreground = new SolidColorBrush(pairedDevice ? (System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#32ADE6") : (System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#38383A"));
                Step3Text.Foreground = new SolidColorBrush(pairedDevice ? (System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#FFFFFF") : (System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#8E8E93"));
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
            [System.Text.Json.Serialization.JsonPropertyName("trusted")]
            public bool Trusted { get; set; }
            [System.Text.Json.Serialization.JsonPropertyName("connected")]
            public bool IsConnected { get; set; }
        }

        private void LoadSettingsView()
        {
            HideAllViews();
            if (SettingsView != null) AnimateView(SettingsView);
            
            LoadSettings();
        }

        private void LoadSettings()
        {
            using var runKey = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(@"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", false);
            if (runKey != null)
            {
                var val = runKey.GetValue("Deskdrop");
                ChkLaunchOnStartup.IsChecked = val != null;
            }

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

            try
            {
                using var runKey = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(@"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", true);
                if (runKey != null)
                {
                    if (ChkLaunchOnStartup.IsChecked == true)
                    {
                        var exePath = System.Diagnostics.Process.GetCurrentProcess().MainModule?.FileName;
                        if (!string.IsNullOrEmpty(exePath))
                        {
                            runKey.SetValue("Deskdrop", $"\"{exePath}\" --hidden");
                        }
                    }
                    else
                    {
                        runKey.DeleteValue("Deskdrop", false);
                    }
                }
            }
            catch { /* Ignore */ }

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
            
            ShowToast("Settings saved");
        }

        private void BorderPushClipboard_Click(object sender, RoutedEventArgs e)
        {
            System.Threading.Tasks.Task.Run(() =>
            {
                DaemonClient.Send(new { cmd = "push_clipboard" });
            });
        }

        private void BorderSendFiles_Click(object sender, RoutedEventArgs e)
        {
            var dlg = new Microsoft.Win32.OpenFileDialog();
            dlg.Multiselect = false;
            dlg.Title = "Select File to Send";
            if (dlg.ShowDialog() == true)
            {
                var file = dlg.FileName;
                _clipboardManager?.PushFile(file);
                ShowToast($"Sending file: {System.IO.Path.GetFileName(file)}...");
            }
        }

        private void BorderStreamCamera_Click(object sender, RoutedEventArgs e)
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
                        ShowToast("Deskdrop engine is unreachable.", true);
                    }
                    else
                    {
                        ShowToast($"Connecting to {addr}...");
                    }
                });
            });
        }

        private async void BorderBroadcastCamera_Click(object sender, RoutedEventArgs e)
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
                BorderBroadcastCameraBtn.Background = new SolidColorBrush(System.Windows.Media.Color.FromArgb(0xFF, 0xFF, 0xEB, 0xEC)); // #FFEBEC
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
                    BorderBroadcastCameraBtn.Background = new SolidColorBrush(System.Windows.Media.Color.FromArgb(0xFF, 0xFF, 0xCD, 0xD2)); // Stronger red tint
                }
                catch (Exception ex)
                {
                    _isBroadcasting = false;
                    _cameraPublisher?.Dispose();
                    _cameraPublisher = null;
                    
                    TxtBroadcastTitle.Text = "Broadcast Camera";
                    ShowToast($"Camera error: {ex.Message}", true);
                }
            }
        }

        private void BtnShowQR_Click(object sender, RoutedEventArgs e)
        {
            var qrWindow = new QRPairingWindow();
            qrWindow.Owner = this;
            qrWindow.ShowDialog();
        }

        private string? _currentTofuDeviceId;

        public void ShowTofuPrompt(string deviceId, string deviceName, string fingerprint)
        {
            Dispatcher.Invoke(() =>
            {
                _currentTofuDeviceId = deviceId;
                TxtTofuDeviceName.Text = deviceName;
                TxtTofuFingerprint.Text = FormatFingerprint(fingerprint);
                TofuPromptOverlay.Visibility = Visibility.Visible;
            });
        }

        private void BtnTofuTrust_Click(object sender, RoutedEventArgs e)
        {
            if (_currentTofuDeviceId != null)
            {
                _clipboardManager?.RespondToTrust(_currentTofuDeviceId, true);
                ShowToast($"Trusted {TxtTofuDeviceName.Text}");
                _currentTofuDeviceId = null;
            }
            TofuPromptOverlay.Visibility = Visibility.Collapsed;
        }

        private void BtnTofuReject_Click(object sender, RoutedEventArgs e)
        {
            if (_currentTofuDeviceId != null)
            {
                _clipboardManager?.RespondToTrust(_currentTofuDeviceId, false);
                _currentTofuDeviceId = null;
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

        private void Grid_DragEnter(object sender, System.Windows.DragEventArgs e)
        {
            if (e.Data.GetDataPresent(System.Windows.DataFormats.FileDrop))
            {
                DropZoneOverlay.Visibility = Visibility.Visible;
                e.Effects = System.Windows.DragDropEffects.Copy;
            }
            else
            {
                e.Effects = System.Windows.DragDropEffects.None;
            }
        }

        private void Grid_DragLeave(object sender, System.Windows.DragEventArgs e)
        {
            DropZoneOverlay.Visibility = Visibility.Collapsed;
        }

        private void Grid_Drop(object sender, System.Windows.DragEventArgs e)
        {
            DropZoneOverlay.Visibility = Visibility.Collapsed;
            if (e.Data.GetDataPresent(System.Windows.DataFormats.FileDrop))
            {
                string[] files = (string[])e.Data.GetData(System.Windows.DataFormats.FileDrop);
                System.Threading.Tasks.Task.Run(() =>
                {
                    foreach (var file in files)
                    {
                        _clipboardManager.PushFile(file);
                    }
                });
                ShowToast($"Sending {files.Length} file(s)...");
            }
        }
        
        private void TransferHistoryItem_Click(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            // Placeholder for clicking a transfer history item
        }
    }
}
