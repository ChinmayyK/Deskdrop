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
        private bool _hasCompletedOnboarding = false;
        private string _activeCallDeviceId = "";

        public MainWindow(ClipboardManager clipboardManager)
        {
            InitializeComponent();
            _clipboardManager = clipboardManager;
            _clipboardManager.HistoryItemAdded += OnHistoryItemAdded;
            _clipboardManager.QuickContextUpdated += OnQuickContextUpdated;
            _clipboardManager.QuickContextUpdated += OnQuickContextUpdated;
            LoadTransfersView();
            
            // Bind UI lists to the global store
            if (ActiveTransfersList != null) ActiveTransfersList.ItemsSource = DeskdropStore.Shared.ActiveTransfers;
            if (DevicesList != null) DevicesList.ItemsSource = DeskdropStore.Shared.Peers;
            
            DeskdropStore.Shared.PropertyChanged += (s, e) => {
                if (e.PropertyName == nameof(DeskdropStore.IsDaemonRunning) || e.PropertyName == nameof(DeskdropStore.Peers))
                {
                    Dispatcher.Invoke(() => {
                        UpdateOnboardingStatus(DeskdropStore.Shared.Peers.ToList());
                        RefreshDiagnosticsStateUI();
                    });
                }
            };
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
            if (TryFindResource("FadeInTransition") is System.Windows.Media.Animation.Storyboard sb)
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
            if (ActivityView != null) ActivityView.Visibility = Visibility.Collapsed;
            if (DevicesView != null) DevicesView.Visibility = Visibility.Collapsed;
            if (SettingsView != null) SettingsView.Visibility = Visibility.Collapsed;
            if (DiagnosticsView != null) DiagnosticsView.Visibility = Visibility.Collapsed;
            if (TransfersView != null) TransfersView.Visibility = Visibility.Collapsed;
        }

        private void NavActivity_Click(object sender, RoutedEventArgs e)
        {
            LoadActivityView();
        }

        private void LoadActivityView()
        {
            HideAllViews();
            if (ActivityView != null) AnimateView(ActivityView);
            if (ActivityFeedList != null)
            {
                ActivityFeedList.ItemsSource = _clipboardManager.GetHistory().ToList();
            }
        }

        private void OnQuickContextUpdated(string? text)
        {
            // Logic moved to QuickAccessWindow
        }

        private void OnHistoryItemAdded(HistoryItem obj)
        {
            // Logic handled by DeskdropStore binding
        }

        protected override void OnMouseLeftButtonDown(MouseButtonEventArgs e)
        {
            base.OnMouseLeftButtonDown(e);
            if (e.ButtonState == MouseButtonState.Pressed)
            {
                DragMove();
            }
        }

        protected override void OnKeyDown(System.Windows.Input.KeyEventArgs e)
        {
            base.OnKeyDown(e);
            
            // Toggle Command Palette on Ctrl+K
            if (e.Key == Key.K && (Keyboard.Modifiers & ModifierKeys.Control) == ModifierKeys.Control)
            {
                ToggleCommandPalette();
                e.Handled = true;
                return;
            }

            // If Command Palette is open, handle navigation and enter/escape
            if (CommandPaletteOverlay.Visibility == Visibility.Visible)
            {
                if (e.Key == Key.Escape)
                {
                    CommandPaletteOverlay.Visibility = Visibility.Collapsed;
                    e.Handled = true;
                }
                else if (e.Key == Key.Down)
                {
                    if (CommandList.SelectedIndex < CommandList.Items.Count - 1)
                        CommandList.SelectedIndex++;
                    e.Handled = true;
                }
                else if (e.Key == Key.Up)
                {
                    if (CommandList.SelectedIndex > 0)
                        CommandList.SelectedIndex--;
                    e.Handled = true;
                }
                else if (e.Key == Key.Enter)
                {
                    ExecuteSelectedCommand();
                    e.Handled = true;
                }
            }
        }

        public void ToggleCommandPaletteGlobal()
        {
            if (WindowState == WindowState.Minimized)
            {
                WindowState = WindowState.Normal;
            }
            Activate();
            ToggleCommandPalette();
        }

        private void ToggleCommandPalette()
        {
            if (CommandPaletteOverlay.Visibility == Visibility.Visible)
            {
                CommandPaletteOverlay.Visibility = Visibility.Collapsed;
            }
            else
            {
                CommandPaletteOverlay.Visibility = Visibility.Visible;
                TxtCommandInput.Text = "";
                RefreshCommandList();
                TxtCommandInput.Focus();
            }
        }

        private void TxtCommandInput_TextChanged(object sender, TextChangedEventArgs e)
        {
            RefreshCommandList();
        }

        private void RefreshCommandList()
        {
            var query = TxtCommandInput.Text.ToLowerInvariant();
            var allCommands = new System.Collections.Generic.List<PaletteCommand>
            {
                new PaletteCommand { Title = "Send a File", Icon = "📎", Action = "SendFile" },
                new PaletteCommand { Title = "Show Magic Link (QR)", Icon = "📱", Action = "ShowQR" },
                new PaletteCommand { Title = "View Diagnostics", Icon = "🔧", Action = "Diagnostics" },
                new PaletteCommand { Title = "Settings", Icon = "⚙", Action = "Settings" },
                new PaletteCommand { Title = "Quit Deskdrop", Icon = "🛑", Action = "Quit" }
            };

            foreach (var peer in DeskdropStore.Shared.Peers)
            {
                allCommands.Insert(0, new PaletteCommand {
                    Title = $"Send Clipboard to {peer.friendly_name}",
                    Icon = "📋",
                    Action = "SendClipboardToTarget",
                    Target = peer.device_id
                });
            }

            var filtered = string.IsNullOrWhiteSpace(query)
                ? allCommands
                : allCommands.Where(c => c.Title.ToLowerInvariant().Contains(query)).ToList();

            CommandList.ItemsSource = filtered;
            if (filtered.Count > 0)
                CommandList.SelectedIndex = 0;
        }

        private void ExecuteSelectedCommand()
        {
            if (CommandList.SelectedItem is PaletteCommand cmd)
            {
                CommandPaletteOverlay.Visibility = Visibility.Collapsed;
                switch (cmd.Action)
                {
                    case "SendFile":
                        var dlg = new Microsoft.Win32.OpenFileDialog { Multiselect = true };
                        if (dlg.ShowDialog() == true)
                        {
                            System.Threading.Tasks.Task.Run(() =>
                            {
                                foreach (var file in dlg.FileNames)
                                    _clipboardManager.PushFile(file);
                            });
                            ShowToast($"Sending {dlg.FileNames.Length} file(s)...");
                        }
                        break;
                    case "ShowQR":
                        new QRPairingWindow().Show();
                        break;
                    case "Diagnostics":
                        LoadDiagnosticsView();
                        break;
                    case "Settings":
                        LoadSettingsView();
                        break;
                    case "Quit":
                        System.Windows.Application.Current.Shutdown();
                        break;
                    case "SendClipboardToTarget":
                        System.Threading.Tasks.Task.Run(() => 
                        {
                            var clipboardText = "";
                            Dispatcher.Invoke(() => {
                                if (System.Windows.Forms.Clipboard.ContainsText())
                                    clipboardText = System.Windows.Forms.Clipboard.GetText();
                            });
                            
                            if (!string.IsNullOrEmpty(clipboardText))
                            {
                                DaemonClient.Send(new {
                                    cmd = "push_clipboard",
                                    target_device = cmd.Target,
                                    text = clipboardText
                                });
                                Dispatcher.Invoke(() => ShowToast("Clipboard sent."));
                            }
                        });
                        break;
                }
            }
        }

        private void CommandList_MouseDoubleClick(object sender, MouseButtonEventArgs e)
        {
            ExecuteSelectedCommand();
        }

        // Polling timer removed. State is now managed by DeskdropStore.

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
            
            RefreshDiagnosticsStateUI();
        }

        private void RefreshDiagnosticsStateUI()
        {
            bool isRunning = DeskdropStore.Shared.IsDaemonRunning;
            int peerCount = DeskdropStore.Shared.Peers.Count;

            if (TxtDiagDaemonStatus != null)
            {
                TxtDiagDaemonStatus.Text = isRunning ? "Running" : "Stopped";
                TxtDiagDaemonSuggestion.Visibility = isRunning ? Visibility.Collapsed : Visibility.Visible;
                BtnRestartConnection.Visibility = isRunning ? Visibility.Collapsed : Visibility.Visible;
            }

            if (isRunning && TxtMetricsContent != null)
            {
                System.Threading.Tasks.Task.Run(() =>
                {
                    var doc = DaemonClient.GetMetrics();
                    if (doc != null && doc.RootElement.TryGetProperty("data", out var data))
                    {
                        var json = System.Text.Json.JsonSerializer.Serialize(data, new System.Text.Json.JsonSerializerOptions { WriteIndented = true });
                        Dispatcher.Invoke(() => TxtMetricsContent.Text = json);
                    }
                    else
                    {
                        Dispatcher.Invoke(() => TxtMetricsContent.Text = "No metrics available.");
                    }
                });
            }
        }

        private void BtnRestartConnection_Click(object sender, RoutedEventArgs e)
        {
            var exePath = System.Diagnostics.Process.GetCurrentProcess().MainModule?.FileName;
            if (string.IsNullOrEmpty(exePath)) return;
            System.Diagnostics.Process.Start(exePath);
            System.Windows.Application.Current.Shutdown();
        }

        private void BtnExportBundle_Click(object sender, RoutedEventArgs e)
        {
            try
            {
                var dialog = new Microsoft.Win32.SaveFileDialog
                {
                    Filter = "Zip Archive|*.zip",
                    Title = "Export Support Bundle",
                    FileName = $"deskdrop-support-{DateTime.Now:yyyyMMddHHmmss}.zip"
                };

                if (dialog.ShowDialog() == true)
                {
                    var appDataDir = System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Deskdrop");
                    if (System.IO.Directory.Exists(appDataDir))
                    {
                        if (System.IO.File.Exists(dialog.FileName))
                            System.IO.File.Delete(dialog.FileName);
                        System.IO.Compression.ZipFile.CreateFromDirectory(appDataDir, dialog.FileName, System.IO.Compression.CompressionLevel.Fastest, false);
                        ShowToast("Support bundle exported successfully.");
                    }
                    else
                    {
                        ShowToast("No logs found to export.", true);
                    }
                }
            }
            catch (Exception ex)
            {
                ShowToast($"Failed to export bundle: {ex.Message}", true);
            }
        }

        private void BtnScanAgain_Click(object sender, RoutedEventArgs e)
        {
            DaemonClient.Send(new { cmd = "rescan_peers" });
            RefreshDiagnosticsStateUI();
        }

        public void ShowToast(string message, bool isError = false)
        {
            NotificationHelper.ShowToast(isError ? "Deskdrop Error" : "Deskdrop", message);
        }


        private void UpdateOnboardingVisibility()
        {
            // Removed Onboarding and QuickActionsRibbon during Dashboard redesign
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
            
            RefreshDevicesListUI();
        }

        private void RefreshDevicesListUI()
        {
            var peers = DeskdropStore.Shared.Peers.ToList();
            var activeCall = DeskdropStore.Shared.ActiveCall;

            Dispatcher.Invoke(() =>
            {
                if (!_hasCompletedOnboarding && peers != null)
                {
                    UpdateOnboardingStatus(peers);
                }

                if (activeCall != null && activeCall.state == "incoming" && IncomingCallBanner != null)
                {
                    _activeCallDeviceId = activeCall.device_id;
                    TxtCallTitle.Text = string.IsNullOrEmpty(activeCall.contact_name) ? $"Incoming call from {activeCall.number}" : $"Incoming call from {activeCall.contact_name}";
                    if (string.IsNullOrEmpty(activeCall.number) && string.IsNullOrEmpty(activeCall.contact_name))
                    {
                        TxtCallTitle.Text = "Incoming Camera Stream";
                    }
                    TxtCallSubtitle.Text = $"Via {activeCall.device_name}";
                    IncomingCallBanner.Visibility = Visibility.Visible;
                }
                else if (IncomingCallBanner != null)
                {
                    IncomingCallBanner.Visibility = Visibility.Collapsed;
                }
            });
        }

        private void UpdateOnboardingStatus(System.Collections.Generic.List<PeerViewModel> peers)
        {
            if (_hasCompletedOnboarding) return;
            
            bool foundDevice = peers.Count > 0;
            if (!foundDevice)
            {
                if (System.Windows.Application.Current.Windows.OfType<OnboardingWindow>().Count() == 0)
                {
                    var ob = new OnboardingWindow();
                    ob.Closed += (s, e) => 
                    {
                        if (ob.Success)
                        {
                            _hasCompletedOnboarding = true;
                            // Optionally persist to settings here or rely on Daemon
                        }
                    };
                    ob.Show();
                }
            }
            else
            {
                _hasCompletedOnboarding = true;
                foreach (var w in System.Windows.Application.Current.Windows.OfType<OnboardingWindow>().ToList())
                {
                    w.Close();
                }
            }
        }

        private void BtnRenameDevice_Click(object sender, RoutedEventArgs e)
        {
            if (sender is FrameworkElement el && el.Tag is string deviceId)
            {
                var peer = DeskdropStore.Shared.Peers.FirstOrDefault(p => p.device_id == deviceId);
                if (peer != null)
                {
                    string newName = peer.friendly_name + " (Renamed)";
                    System.Threading.Tasks.Task.Run(() => DaemonClient.RenameTrustedDevice(deviceId, newName));
                }
            }
        }

        private void BtnPauseSyncDevice_Click(object sender, RoutedEventArgs e)
        {
            if (sender is FrameworkElement el && el.Tag is string deviceId)
            {
                System.Threading.Tasks.Task.Run(() => DaemonClient.PauseSyncPeer(deviceId));
            }
        }

        private void BtnForgetDevice_Click(object sender, RoutedEventArgs e)
        {
            if (sender is FrameworkElement el && el.Tag is string deviceId)
            {
                System.Threading.Tasks.Task.Run(() => DaemonClient.ForgetDevice(deviceId));
            }
        }

        private void BtnDisconnectDevice_Click(object sender, RoutedEventArgs e)
        {
            if (sender is System.Windows.Controls.Button btn && btn.Tag is string deviceId)
            {
                System.Threading.Tasks.Task.Run(() =>
                {
                    DaemonClient.Send(new { cmd = "disconnect_peer", device_id = deviceId });
                    RefreshDevicesListUI();
                });
            }
        }

        private void BtnAcceptCall_Click(object sender, RoutedEventArgs e)
        {
            if (!string.IsNullOrEmpty(_activeCallDeviceId))
            {
                DaemonClient.Send(new { cmd = "call_action", action = "accept", target_device = _activeCallDeviceId });
                IncomingCallBanner.Visibility = Visibility.Collapsed;
                
                // Open CameraPreviewWindow
                Dispatcher.Invoke(() =>
                {
                    var cameraWindow = new CameraPreviewWindow();
                    cameraWindow.Show();
                });
            }
        }

        private void BtnDeclineCall_Click(object sender, RoutedEventArgs e)
        {
            if (!string.IsNullOrEmpty(_activeCallDeviceId))
            {
                DaemonClient.Send(new { cmd = "call_action", action = "reject", target_device = _activeCallDeviceId });
                IncomingCallBanner.Visibility = Visibility.Collapsed;
            }
        }
        
        public class PaletteCommand
        {
            public string Title { get; set; } = "";
            public string Icon { get; set; } = "";
            public string Action { get; set; } = "";
            public string Target { get; set; } = "";
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
            if (key != null)
            {
                ChkEnableHotkeys.IsChecked = (int?)key.GetValue("EnableHotkeys", 1) == 1;
            }

            System.Threading.Tasks.Task.Run(() =>
            {
                var settingsDoc = DaemonClient.Send(new { cmd = "get_settings" });
                if (settingsDoc != null && settingsDoc.RootElement.TryGetProperty("settings", out var settings))
                {
                    Dispatcher.Invoke(() =>
                    {
                        if (settings.TryGetProperty("sync_enabled", out var sync)) ChkSyncEnabled.IsChecked = sync.GetBoolean();
                        if (settings.TryGetProperty("show_receive_notification", out var notif)) ChkShowNotifications.IsChecked = notif.GetBoolean();
                        if (settings.TryGetProperty("require_tofu_confirmation", out var tofu)) ChkRequireTofu.IsChecked = tofu.GetBoolean();
                        if (settings.TryGetProperty("device_name", out var devName)) TxtDeviceName.Text = devName.GetString() ?? "";
                    });
                }
            });
        }

        private void BtnSaveSettings_Click(object sender, RoutedEventArgs e)
        {
            using var key = Microsoft.Win32.Registry.CurrentUser.CreateSubKey(@"Software\Deskdrop");
            key.SetValue("EnableHotkeys", ChkEnableHotkeys.IsChecked == true ? 1 : 0, Microsoft.Win32.RegistryValueKind.DWord);

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
                    device_name = string.IsNullOrWhiteSpace(TxtDeviceName.Text) ? null : TxtDeviceName.Text,
                    require_tofu_confirmation = ChkRequireTofu.IsChecked == true,
                    show_receive_notification = ChkShowNotifications.IsChecked == true,
                });
            });
            
            ShowToast("Settings saved");
        }

        private void BtnInstallContextMenu_Click(object sender, RoutedEventArgs e)
        {
            try
            {
                var exePath = System.Diagnostics.Process.GetCurrentProcess().MainModule?.FileName;
                if (string.IsNullOrEmpty(exePath)) return;

                // Add to HKEY_CURRENT_USER\Software\Classes\*\shell\Deskdrop
                using var key = Microsoft.Win32.Registry.CurrentUser.CreateSubKey(@"Software\Classes\*\shell\Deskdrop");
                key.SetValue("", "Send via Deskdrop");
                key.SetValue("Icon", $"\"{exePath}\",0");

                using var commandKey = key.CreateSubKey("command");
                commandKey.SetValue("", $"\"{exePath}\" --push-file \"%1\"");

                // Register deskdrop:// protocol
                using var uriKey = Microsoft.Win32.Registry.CurrentUser.CreateSubKey(@"Software\Classes\deskdrop");
                uriKey.SetValue("", "URL:Deskdrop Protocol");
                uriKey.SetValue("URL Protocol", "");
                using var uriCmdKey = uriKey.CreateSubKey(@"shell\open\command");
                uriCmdKey.SetValue("", $"\"{exePath}\" \"%1\"");

                ShowToast("Context menu & protocol installed successfully!");
            }
            catch (UnauthorizedAccessException)
            {
                System.Windows.MessageBox.Show("Please run Deskdrop as Administrator to install the Context Menu.", "Permission Denied", MessageBoxButton.OK, MessageBoxImage.Warning);
            }
            catch (Exception ex)
            {
                ShowToast($"Failed to install context menu: {ex.Message}", true);
            }
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
                    _cameraPublisher.Dispose();
                    _cameraPublisher = null;
                }
                
                // TxtBroadcastTitle.Text = "Broadcast Camera";
                // BorderBroadcastCameraBtn.Background = new SolidColorBrush(System.Windows.Media.Color.FromArgb(0xFF, 0xFF, 0xEB, 0xEC)); // #FFEBEC
            }
            else
            {
                try
                {
                    _isBroadcasting = true;
                    // TxtBroadcastTitle.Text = "Starting...";
                    
                    _cameraPublisher = new CameraPublisher(_clipboardManager);
                    await _cameraPublisher.StartBroadcastingAsync();
                    
                    // TxtBroadcastTitle.Text = "Stop Broadcasting";
                    // BorderBroadcastCameraBtn.Background = new SolidColorBrush(System.Windows.Media.Color.FromArgb(0xFF, 0xFF, 0xCD, 0xD2)); // Stronger red tint
                }
                catch (Exception ex)
                {
                    _isBroadcasting = false;
                    _cameraPublisher?.Dispose();
                    _cameraPublisher = null;
                    
                    // TxtBroadcastTitle.Text = "Broadcast Camera";
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


        public void ShowTofuPrompt(string deviceId, string deviceName, string fingerprint)
        {
            Dispatcher.Invoke(() =>
            {
                var msg = $"Incoming connection from {deviceName}\nFingerprint: {FormatFingerprint(fingerprint)}";
                NotificationHelper.ShowToastWithActions(
                    "Trust Device?", 
                    msg, 
                    null,
                    $"deskdrop://tofu?action=accept&device_id={System.Uri.EscapeDataString(deviceId)}",
                    $"deskdrop://tofu?action=reject&device_id={System.Uri.EscapeDataString(deviceId)}"
                );
            });
        }

        private static string FormatFingerprint(string raw)
        {
            var clean = raw.Replace(":", "").ToUpperInvariant();
            var pairs = new System.Collections.Generic.List<string>();
            for (int i = 0; i + 1 < clean.Length; i += 2)
                pairs.Add(clean.Substring(i, 2));
            var lines = new System.Collections.Generic.List<string>();
            for (int i = 0; i < pairs.Count; i += 8)
            {
                var chunk = pairs.Skip(i).Take(8);
                lines.Add(string.Join(":", chunk));
            }
            return string.Join("\n", lines);
        }

        // --- NEW MISSING METHODS FOR XAML ---

        public static string GetLocalIPAddress()
        {
            try
            {
                var host = System.Net.Dns.GetHostEntry(System.Net.Dns.GetHostName());
                foreach (var ip in host.AddressList)
                {
                    if (ip.AddressFamily == System.Net.Sockets.AddressFamily.InterNetwork)
                    {
                        return ip.ToString();
                    }
                }
            }
            catch { }
            return "127.0.0.1";
        }

        private void BtnShowQRCode_Click(object sender, RoutedEventArgs e)
        {
            var qrWindow = new QRPairingWindow();
            qrWindow.Owner = this;
            qrWindow.ShowDialog();
        }

        private void BtnSendFile_Click(object sender, RoutedEventArgs e)
        {
            var dlg = new Microsoft.Win32.OpenFileDialog { Multiselect = true };
            if (dlg.ShowDialog() == true)
            {
                System.Threading.Tasks.Task.Run(() =>
                {
                    foreach (var file in dlg.FileNames)
                        _clipboardManager.PushFile(file);
                });
                ShowToast($"Sending {dlg.FileNames.Length} file(s)...");
            }
        }

        private void CommandPaletteBackdrop_Click(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            CommandPaletteOverlay.Visibility = Visibility.Collapsed;
        }



        private void TxtCommandInput_PreviewKeyDown(object sender, System.Windows.Input.KeyEventArgs e)
        {
            if (e.Key == Key.Down)
            {
                if (CommandList.SelectedIndex < CommandList.Items.Count - 1)
                    CommandList.SelectedIndex++;
                e.Handled = true;
            }
            else if (e.Key == Key.Up)
            {
                if (CommandList.SelectedIndex > 0)
                    CommandList.SelectedIndex--;
                e.Handled = true;
            }
            else if (e.Key == Key.Enter)
            {
                ExecuteSelectedCommand();
                e.Handled = true;
            }
            else if (e.Key == Key.Escape)
            {
                CommandPaletteOverlay.Visibility = Visibility.Collapsed;
                e.Handled = true;
            }
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
        
        private void BtnTransferPrimaryAction_Click(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is FileTransferState transfer)
            {
                System.Threading.Tasks.Task.Run(() =>
                {
                    try
                    {
                        if (transfer.status == "incoming") DaemonClient.AcceptFileTransfer(transfer.transfer_id);
                        else if (transfer.status == "in_progress") DaemonClient.PauseFileTransfer(transfer.transfer_id);
                        else if (transfer.status == "paused") DaemonClient.ResumeFileTransfer(transfer.transfer_id);
                        else if (transfer.status == "completed" && !string.IsNullOrEmpty(transfer.destination))
                        {
                            System.Diagnostics.Process.Start("explorer.exe", $"/select,\"{transfer.destination}\"");
                        }
                    }
                    catch (Exception ex)
                    {
                        System.Windows.Application.Current?.Dispatcher.Invoke(() => ShowToast($"Transfer action failed: {ex.Message}"));
                    }
                });
            }
        }

        private void BtnTransferSecondaryAction_Click(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is FileTransferState transfer)
            {
                System.Threading.Tasks.Task.Run(() =>
                {
                    try
                    {
                        if (transfer.status == "incoming") DaemonClient.RejectFileTransfer(transfer.transfer_id, "User rejected");
                        else if (transfer.status == "in_progress" || transfer.status == "paused") DaemonClient.CancelFileTransfer(transfer.transfer_id);
                    }
                    catch (Exception ex)
                    {
                        System.Windows.Application.Current?.Dispatcher.Invoke(() => ShowToast($"Transfer action failed: {ex.Message}"));
                    }
                });
            }
        }

        private void ActivityFeedItem_Click(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is HistoryItem item)
            {
                if (item.TypeIcon == "📎")
                {
                    if (System.IO.File.Exists(item.FullText))
                    {
                        System.Diagnostics.Process.Start("explorer.exe", $"/select,\"{item.FullText}\"");
                    }
                    else if (System.IO.Directory.Exists(item.FullText))
                    {
                        System.Diagnostics.Process.Start("explorer.exe", $"\"{item.FullText}\"");
                    }
                }
                else
                {
                    System.Windows.Clipboard.SetText(item.FullText);
                    ShowToast("Copied to clipboard.");
                }
            }
        }

        private void TransferHistoryItem_Click(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            ActivityFeedItem_Click(sender, e);
        }
    }
}
