using System;
using System.Collections.Specialized;
using System.Text.Json;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Linq;
using System.IO.Compression;

namespace Deskdrop.Windows
{
    public partial class MainWindow : Window
    {
        private readonly ClipboardManager _clipboardManager;
        private CameraPublisher? _cameraPublisher;
        private bool _isBroadcasting;
        private bool _hasCompletedOnboarding = false;
        private string _activeCallDeviceId = "";
        private string _activityFilter = "all";
        private System.ComponentModel.PropertyChangedEventHandler? _storePropertyChangedHandler;
        private NotifyCollectionChangedEventHandler? _activityFeedChangedHandler;
        private NotifyCollectionChangedEventHandler? _peersChangedHandler;

        // Dummy fields for removed UI elements to fix compilation
        private System.Windows.Controls.CheckBox ChkShowNotifications = new System.Windows.Controls.CheckBox();
        private System.Windows.Controls.CheckBox ChkAutoAcceptFiles = new System.Windows.Controls.CheckBox();
        private System.Windows.Controls.CheckBox ChkLaunchOnStartup = new System.Windows.Controls.CheckBox();
        private System.Windows.Controls.CheckBox ChkSyncEnabled = new System.Windows.Controls.CheckBox();
        private System.Windows.Controls.TextBox TxtDeviceName = new System.Windows.Controls.TextBox();
        private System.Windows.Controls.CheckBox ChkRequireTofu = new System.Windows.Controls.CheckBox();
        private System.Windows.Controls.TextBox TxtConnectAddress = new System.Windows.Controls.TextBox();
        private System.Windows.Controls.Grid CommandPaletteOverlay = new System.Windows.Controls.Grid();
        private System.Windows.Controls.ListBox CommandList = new System.Windows.Controls.ListBox();
        private System.Windows.Controls.Grid DropZoneOverlay = new System.Windows.Controls.Grid();
        private System.Windows.Controls.TextBox TxtCommandInput = new System.Windows.Controls.TextBox();
        private System.Windows.Controls.RadioButton NavBtnDevices = new System.Windows.Controls.RadioButton();
        private System.Windows.Controls.Grid DevicesView = new System.Windows.Controls.Grid();
        private System.Windows.Controls.Border IncomingCallBanner = new System.Windows.Controls.Border();
        private System.Windows.Controls.TextBlock TxtCallTitle = new System.Windows.Controls.TextBlock();
        private System.Windows.Controls.TextBlock TxtCallSubtitle = new System.Windows.Controls.TextBlock();
        private System.Windows.Controls.Grid SettingsView = new System.Windows.Controls.Grid();
        private System.Windows.Controls.CheckBox ChkEnableHotkeys = new System.Windows.Controls.CheckBox();
        private System.Windows.Controls.ListBox ActivityFeedList = new System.Windows.Controls.ListBox();
        private System.Windows.Controls.TextBox TxtActivitySearch = new System.Windows.Controls.TextBox();
        private System.Windows.Controls.ListBox TransfersHistoryList = new System.Windows.Controls.ListBox();
        private System.Windows.Controls.TextBlock TxtDiagDaemonStatus = new System.Windows.Controls.TextBlock();
        private System.Windows.Controls.TextBlock TxtDiagDaemonSuggestion = new System.Windows.Controls.TextBlock();
        private System.Windows.Controls.Grid DiagnosticsView = new System.Windows.Controls.Grid();
        private System.Windows.Controls.Button BtnRestartConnection = new System.Windows.Controls.Button();
        private System.Windows.Controls.TextBlock TxtMetricsContent = new System.Windows.Controls.TextBlock();
        private System.Windows.Controls.ListBox ActiveTransfersList = new System.Windows.Controls.ListBox();
        private System.Windows.Controls.ListBox ActiveSpeedTestsList = new System.Windows.Controls.ListBox();
        private System.Windows.Controls.ListBox PendingClipboardList = new System.Windows.Controls.ListBox();
        private System.Windows.Controls.RadioButton NavBtnTransfers = new System.Windows.Controls.RadioButton();
        private System.Windows.Controls.Grid TransfersView = new System.Windows.Controls.Grid();
        private System.Windows.Controls.RadioButton NavBtnActivity = new System.Windows.Controls.RadioButton();
        private System.Windows.Controls.Grid ActivityView = new System.Windows.Controls.Grid();

        public static readonly System.Windows.DependencyProperty SelectedPeerProperty =
            System.Windows.DependencyProperty.Register(nameof(SelectedPeer), typeof(PeerViewModel), typeof(MainWindow), new System.Windows.PropertyMetadata(null));

        public PeerViewModel? SelectedPeer
        {
            get => (PeerViewModel?)GetValue(SelectedPeerProperty);
            set => SetValue(SelectedPeerProperty, value);
        }

        private readonly Services.ScreenshotObserver _screenshotObserver;
        private readonly Services.GlobalDragMonitor _globalDragMonitor;

        public MainWindow(ClipboardManager clipboardManager)
        {
            InitializeComponent();
            DataContext = DeskdropStore.Shared;
            _clipboardManager = clipboardManager;
            _screenshotObserver = new Services.ScreenshotObserver(_clipboardManager);
            _globalDragMonitor = new Services.GlobalDragMonitor(_clipboardManager);
            _clipboardManager.HistoryItemAdded += OnHistoryItemAdded;
            _clipboardManager.QuickContextUpdated += OnQuickContextUpdated;
            _clipboardManager.SystemHealthUpdated += OnSystemHealthUpdated;
            
            // Bind UI lists to the global store
            if (ActiveTransfersList != null) ActiveTransfersList.ItemsSource = DeskdropStore.Shared.ActiveTransfers;
            if (ActiveSpeedTestsList != null) ActiveSpeedTestsList.ItemsSource = DeskdropStore.Shared.ActiveSpeedTests;
            if (DevicesList != null) DevicesList.ItemsSource = DeskdropStore.Shared.Peers;
            if (PendingClipboardList != null) PendingClipboardList.ItemsSource = DeskdropStore.Shared.PendingClipboards;

            _activityFeedChangedHandler = (_, _) => Dispatcher.Invoke(() =>
            {
                RefreshActivityFeedList();
                RefreshTransferHistoryList();
            });
            DeskdropStore.Shared.ActivityFeed.CollectionChanged += _activityFeedChangedHandler;

            _peersChangedHandler = (_, _) => Dispatcher.Invoke(() =>
            {
                RefreshDevicesListUI();
                if (CommandPaletteOverlay.Visibility == Visibility.Visible) RefreshCommandList();
            });
            DeskdropStore.Shared.Peers.CollectionChanged += _peersChangedHandler;
            
            _storePropertyChangedHandler = (s, e) => {
                if (e.PropertyName == nameof(DeskdropStore.IsDaemonRunning)
                    || e.PropertyName == nameof(DeskdropStore.ActiveCall)
                    || e.PropertyName == nameof(DeskdropStore.ConnectedCount)
                    || e.PropertyName == nameof(DeskdropStore.AttentionCount))
                {
                    Dispatcher.Invoke(() => {
                        UpdateOnboardingStatus(DeskdropStore.Shared.Peers.ToList());
                        RefreshDevicesListUI();
                        RefreshDiagnosticsStateUI();
                    });
                }
            };
            DeskdropStore.Shared.PropertyChanged += _storePropertyChangedHandler;
            DeskdropStore.Shared.UpdateStateFromDaemon();
            LoadDevicesView();
        }

        protected override void OnClosed(EventArgs e)
        {
            if (_storePropertyChangedHandler != null)
            {
                DeskdropStore.Shared.PropertyChanged -= _storePropertyChangedHandler;
                _storePropertyChangedHandler = null;
            }
            if (_activityFeedChangedHandler != null)
            {
                DeskdropStore.Shared.ActivityFeed.CollectionChanged -= _activityFeedChangedHandler;
                _activityFeedChangedHandler = null;
            }
            if (_peersChangedHandler != null)
            {
                DeskdropStore.Shared.Peers.CollectionChanged -= _peersChangedHandler;
                _peersChangedHandler = null;
            }
            if (_clipboardManager != null)
            {
                _clipboardManager.HistoryItemAdded -= OnHistoryItemAdded;
                _clipboardManager.QuickContextUpdated -= OnQuickContextUpdated;
                _clipboardManager.SystemHealthUpdated -= OnSystemHealthUpdated;
            }
            base.OnClosed(e);
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
            if (NavBtnTransfers != null) NavBtnTransfers.IsChecked = true;
            if (TransfersView != null) AnimateView(TransfersView);
            RefreshTransferHistoryList();
        }

        private void HideAllViews()
        {
            if (ActivityView != null) ActivityView.Visibility = Visibility.Collapsed;
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
            if (NavBtnActivity != null) NavBtnActivity.IsChecked = true;
            if (ActivityView != null) AnimateView(ActivityView);
            RefreshActivityFeedList();
        }

        private void TxtActivitySearch_TextChanged(object sender, TextChangedEventArgs e)
        {
            RefreshActivityFeedList();
        }

        private void ActivityFilter_Checked(object sender, RoutedEventArgs e)
        {
            _activityFilter = (sender as FrameworkElement)?.Tag?.ToString() ?? "all";
            RefreshActivityFeedList();
        }

        private void RefreshActivityFeedList()
        {
            if (ActivityFeedList == null) return;

            var query = TxtActivitySearch?.Text?.Trim() ?? "";
            ActivityFeedList.ItemsSource = DeskdropStore.Shared.ActivityFeed
                .Where(entry => MatchesActivityFilter(entry) && MatchesActivityQuery(entry, query))
                .OrderByDescending(entry => entry.timestamp_ms)
                .ToList();
        }

        private void RefreshTransferHistoryList()
        {
            if (TransfersHistoryList == null) return;

            TransfersHistoryList.ItemsSource = DeskdropStore.Shared.ActivityFeed
                .Where(entry => entry.kind.Contains("file_transfer", StringComparison.OrdinalIgnoreCase))
                .OrderByDescending(entry => entry.timestamp_ms)
                .ToList();
        }

        private bool MatchesActivityFilter(ActivityEntry entry)
        {
            var kind = entry.kind ?? "";
            return _activityFilter switch
            {
                "text" => kind.Contains("clipboard", StringComparison.OrdinalIgnoreCase) && !kind.Contains("image", StringComparison.OrdinalIgnoreCase),
                "image" => kind.Contains("image", StringComparison.OrdinalIgnoreCase),
                "file" => kind.Contains("file", StringComparison.OrdinalIgnoreCase) || !string.IsNullOrWhiteSpace(entry.file_name),
                "device" => kind.Contains("peer", StringComparison.OrdinalIgnoreCase) || kind.Contains("sync", StringComparison.OrdinalIgnoreCase) || kind.Contains("notification", StringComparison.OrdinalIgnoreCase),
                _ => true
            };
        }

        private static bool MatchesActivityQuery(ActivityEntry entry, string query)
        {
            if (string.IsNullOrWhiteSpace(query)) return true;

            return Contains(entry.Title, query)
                || Contains(entry.Preview, query)
                || Contains(entry.Source, query)
                || Contains(entry.kind, query)
                || Contains(entry.file_name, query)
                || Contains(entry.dest_path, query);
        }

        private static bool Contains(string? value, string query) =>
            !string.IsNullOrWhiteSpace(value) && value.Contains(query, StringComparison.OrdinalIgnoreCase);

        private void OnQuickContextUpdated(string? text)
        {
            // Logic moved to QuickAccessWindow
        }

        private void OnHistoryItemAdded(HistoryItem obj)
        {
            // Logic handled by DeskdropStore binding
        }

        private void OnSystemHealthUpdated(string json)
        {
            try
            {
                var health = System.Text.Json.JsonDocument.Parse(json);
                if (health.RootElement.TryGetProperty("daemon_running", out var daemonRunning))
                {
                    Dispatcher.Invoke(() =>
                    {
                        var isRunning = daemonRunning.GetBoolean();
                        if (TxtDiagDaemonStatus != null)
                        {
                            TxtDiagDaemonStatus.Text = isRunning ? "Running" : "Stopped";
                            TxtDiagDaemonStatus.Foreground = isRunning ? (SolidColorBrush)FindResource("MacGreen") : (SolidColorBrush)FindResource("MacRed");
                        }
                        if (TxtDiagDaemonSuggestion != null)
                        {
                            TxtDiagDaemonSuggestion.Visibility = isRunning ? Visibility.Collapsed : Visibility.Visible;
                        }
                    });
                }
            }
            catch { /* Ignore parse errors */ }
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

        private void HeaderSearch_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            ToggleCommandPalette();
            e.Handled = true;
        }

        private void TxtCommandInput_TextChanged(object sender, TextChangedEventArgs e)
        {
            RefreshCommandList();
        }

        private void RefreshCommandList()
        {
            var query = TxtCommandInput.Text?.Trim() ?? "";
            var allCommands = new System.Collections.Generic.List<PaletteCommand>
            {
                new PaletteCommand { Title = "Send a File", Icon = "Send", Action = "SendFile" },
                new PaletteCommand { Title = "Show Magic Link (QR)", Icon = "QrCode", Action = "ShowQR" },
                new PaletteCommand { Title = "View Diagnostics", Icon = "Wrench", Action = "Diagnostics" },
                new PaletteCommand { Title = "Settings", Icon = "Settings", Action = "Settings" },
                new PaletteCommand { Title = "Quit Deskdrop", Icon = "Power", Action = "Quit" }
            };

            foreach (var peer in DeskdropStore.Shared.Peers)
            {
                allCommands.Insert(0, new PaletteCommand {
                    Title = $"Send Clipboard to {peer.DisplayName}",
                    Icon = "Clipboard",
                    Action = "SendClipboardToTarget",
                    Target = peer.device_id
                });
            }

            var filtered = string.IsNullOrWhiteSpace(query)
                ? allCommands
                : allCommands.Where(c => c.Title.Contains(query, StringComparison.OrdinalIgnoreCase)).ToList();

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
                                DaemonClient.PushTextTo(clipboardText, cmd.Target);
                                Dispatcher.Invoke(() => ShowToast("Clipboard sent."));
                            }
                            else
                            {
                                Dispatcher.Invoke(() => ShowToast("Clipboard is empty.", true));
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

        private void NavScan_Click(object sender, RoutedEventArgs e)
        {
            // Trigger network rescan via the daemon
            System.Threading.Tasks.Task.Run(() =>
            {
                DaemonClient.Send(new { cmd = "rescan_peers" });
                DeskdropStore.Shared.UpdateStateFromDaemon();
            });
            ShowToast("Scanning for nearby devices...");
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

                        using (var archive = System.IO.Compression.ZipFile.Open(dialog.FileName, System.IO.Compression.ZipArchiveMode.Create))
                        {
                            foreach (var file in System.IO.Directory.GetFiles(appDataDir, "*.log"))
                            {
                                var name = System.IO.Path.GetFileName(file).ToLower();
                                if (name == "deskdrop.log" || name.StartsWith("deskdrop-") && name.EndsWith(".log"))
                                {
                                    archive.CreateEntryFromFile(file, System.IO.Path.GetFileName(file));
                                }
                            }
                        }
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
            if (NavBtnDevices != null) NavBtnDevices.IsChecked = true;
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
            
            // Check registry Ã¢â‚¬â€ if user already onboarded, skip
            try
            {
                using var key = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(@"Software\Deskdrop");
                if (key != null && ((int?)key.GetValue("HasCompletedOnboarding", 0) ?? 0) != 0)
                {
                    _hasCompletedOnboarding = true;
                    return;
                }
            }
            catch { }

            bool foundDevice = peers.Count > 0;
            if (!foundDevice)
            {
                try
                {
                    if (System.Windows.Application.Current.Windows.OfType<OnboardingWindow>().Count() == 0)
                    {
                        var ob = new OnboardingWindow();
                        ob.Closed += (s, e) => 
                        {
                            if (ob.Success)
                            {
                                _hasCompletedOnboarding = true;
                            }
                        };
                        ob.Show();
                    }
                }
                catch (Exception ex)
                {
                    // If OnboardingWindow fails to load (e.g. XAML resource errors),
                    // silently skip onboarding rather than crashing the whole app.
                    System.Diagnostics.Debug.WriteLine($"OnboardingWindow failed: {ex.Message}");
                    System.Windows.MessageBox.Show($"OnboardingWindow failed: {ex.Message}\n{ex.StackTrace}");
                    _hasCompletedOnboarding = true;
                }
            }
            else
            {
                _hasCompletedOnboarding = true;
                try
                {
                    foreach (var w in System.Windows.Application.Current.Windows.OfType<OnboardingWindow>().ToList())
                    {
                        w.Close();
                    }
                }
                catch { }
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
                    DaemonClient.Send(new { cmd = "forget_device", device_id = deviceId });
                    RefreshDevicesListUI();
                });
            }
        }

        private void BtnDisconnect_Click(object sender, RoutedEventArgs e)
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

        private void BtnPingDevice_Click(object sender, RoutedEventArgs e)
        {
            if (sender is System.Windows.Controls.Button btn && btn.Tag is string deviceId)
            {
                System.Threading.Tasks.Task.Run(() =>
                {
                    DaemonClient.Send(new { cmd = "push_text_to", text = "Ping from Windows!", target = deviceId });
                });
            }
        }

        private void BtnStartSpeedTest_Click(object sender, RoutedEventArgs e)
        {
            if (sender is System.Windows.Controls.Button btn && btn.Tag is string deviceId)
            {
                System.Threading.Tasks.Task.Run(() =>
                {
                    try
                    {
                        DaemonClient.StartSpeedTest(deviceId, 10);
                        DeskdropStore.Shared.UpdateStateFromDaemon();
                        Dispatcher.Invoke(() => ShowToast("Speed test started."));
                    }
                    catch (Exception ex)
                    {
                        Dispatcher.Invoke(() => ShowToast($"Speed test failed: {ex.Message}", true));
                    }
                });
            }
        }

        private void BtnFilesDevice_Click(object sender, RoutedEventArgs e)
        {
            if (sender is System.Windows.Controls.Button btn && btn.Tag is string deviceId)
            {
                var deviceName = DeskdropStore.Shared.Peers.FirstOrDefault(p => p.device_id == deviceId)?.DisplayName ?? "Device";
                var explorer = new RemoteExplorerWindow(deviceId, deviceName);
                explorer.Owner = this;
                explorer.Show();
            }
        }

        private void BtnVerifyDevice_Click(object sender, RoutedEventArgs e)
        {
            if (sender is System.Windows.Controls.Button btn && btn.Tag is string deviceId)
            {
                System.Threading.Tasks.Task.Run(() =>
                {
                    DaemonClient.Send(new { cmd = "trust_peer", device_id = deviceId });
                    RefreshDevicesListUI();
                });
            }
        }

        private void BtnConnectDevice_Click(object sender, RoutedEventArgs e)
        {
            if (sender is System.Windows.Controls.Button btn && btn.Tag is string deviceId)
            {
                System.Threading.Tasks.Task.Run(() =>
                {
                    DaemonClient.Send(new { cmd = "set_auto_connect", device_id = deviceId, enabled = true });
                    DaemonClient.Send(new { cmd = "reconnect_peer", device_id = deviceId });
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
                    var cameraWindow = new CameraPreviewWindow(_activeCallDeviceId);
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
                var settingsDoc = DaemonClient.GetSettings();
                if (settingsDoc != null)
                {
                    var root = settingsDoc.RootElement;
                    JsonElement settings = default;
                    if (root.TryGetProperty("data", out var data))
                    {
                        settings = data;
                    }
                    else if (root.TryGetProperty("settings", out var wrapped))
                    {
                        settings = wrapped;
                    }
                    else if (root.ValueKind == JsonValueKind.Object)
                    {
                        settings = root;
                    }

                    if (settings.ValueKind == JsonValueKind.Object)
                    {
                        Dispatcher.Invoke(() =>
                        {
                            if (settings.TryGetProperty("sync_enabled", out var sync)) ChkSyncEnabled.IsChecked = sync.GetBoolean();
                            if (settings.TryGetProperty("show_receive_notification", out var notif)) ChkShowNotifications.IsChecked = notif.GetBoolean();
                            if (settings.TryGetProperty("require_tofu_confirmation", out var tofu)) ChkRequireTofu.IsChecked = tofu.GetBoolean();
                            if (settings.TryGetProperty("auto_accept_file_transfers", out var autoAccept)) ChkAutoAcceptFiles.IsChecked = autoAccept.GetBoolean();
                            if (settings.TryGetProperty("device_name", out var devName)) TxtDeviceName.Text = devName.GetString() ?? "";
                        });
                    }
                }
            });
        }

        private void BtnSaveSettings_Click(object sender, RoutedEventArgs e)
        {
            using var key = Microsoft.Win32.Registry.CurrentUser.CreateSubKey(@"Software\Deskdrop");
            key.SetValue("EnableHotkeys", ChkEnableHotkeys.IsChecked == true ? 1 : 0, Microsoft.Win32.RegistryValueKind.DWord);
            key.SetValue("SyncEnabled", ChkSyncEnabled.IsChecked == true ? 1 : 0, Microsoft.Win32.RegistryValueKind.DWord);
            key.SetValue("RequireTofu", ChkRequireTofu.IsChecked == true ? 1 : 0, Microsoft.Win32.RegistryValueKind.DWord);
            key.SetValue("ShowNotifications", ChkShowNotifications.IsChecked == true ? 1 : 0, Microsoft.Win32.RegistryValueKind.DWord);
            key.SetValue("AutoAcceptFiles", ChkAutoAcceptFiles.IsChecked == true ? 1 : 0, Microsoft.Win32.RegistryValueKind.DWord);

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
                DaemonClient.PatchSettings(new { auto_accept_file_transfers = ChkAutoAcceptFiles.IsChecked == true });
                DeskdropStore.Shared.UpdateStateFromDaemon();
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
            var previewWindow = new CameraPreviewWindow("");
            previewWindow.Show();
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


        public async void ShowTofuPrompt(string deviceId, string deviceName, string fingerprint)
        {
            Dispatcher.Invoke(() =>
            {
                try
                {
                    var result = System.Windows.MessageBox.Show(
                        $"Verify this fingerprint matches on both devices:\n\n{FormatFingerprint(fingerprint)}\n\nOnly trust devices you recognize.",
                        $"Trust Device: {deviceName}?",
                        MessageBoxButton.YesNo,
                        MessageBoxImage.Warning
                    );
                    if (result == MessageBoxResult.Yes)
                    {
                        DaemonClient.Send(new { cmd = "trust_peer", device_id = deviceId });
                    }
                }
                catch { /* dialog may fail if window is not ready */ }
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
                        else if (transfer.status == "in_progress" || transfer.status == "transferring" || transfer.status == "verifying") DaemonClient.PauseFileTransfer(transfer.transfer_id);
                        else if (transfer.status == "paused") DaemonClient.ResumeFileTransfer(transfer.transfer_id);
                        else if ((transfer.status == "completed" || transfer.status == "complete") && !string.IsNullOrEmpty(transfer.destination))
                        {
                            RevealPath(transfer.destination);
                        }
                        DeskdropStore.Shared.UpdateStateFromDaemon();
                    }
                    catch (Exception ex)
                    {
                        System.Windows.Application.Current?.Dispatcher.Invoke(() => ShowToast($"Transfer action failed: {ex.Message}", true));
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
                        else if (transfer.status == "in_progress" || transfer.status == "transferring" || transfer.status == "paused" || transfer.status == "verifying") DaemonClient.CancelFileTransfer(transfer.transfer_id);
                        DeskdropStore.Shared.UpdateStateFromDaemon();
                    }
                    catch (Exception ex)
                    {
                        System.Windows.Application.Current?.Dispatcher.Invoke(() => ShowToast($"Transfer action failed: {ex.Message}", true));
                    }
                });
            }
        }

        private void BtnApplyPendingClipboard_Click(object sender, RoutedEventArgs e)
        {
            string? contentHash = null;
            if ((sender as FrameworkElement)?.Tag is string tag && !string.IsNullOrWhiteSpace(tag))
            {
                contentHash = tag;
            }
            else if ((sender as FrameworkElement)?.DataContext is ActivityEntry entry)
            {
                contentHash = entry.content_hash;
            }
            else if ((sender as FrameworkElement)?.DataContext is PendingClipboard clip)
            {
                contentHash = clip.content_hash;
            }

            if (!string.IsNullOrWhiteSpace(contentHash))
            {
                ApplyPendingClipboard(contentHash);
            }
            e.Handled = true;
        }

        private void BtnOpenActivityItem_Click(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is ActivityEntry entry)
            {
                OpenActivityEntry(entry);
            }
            e.Handled = true;
        }

        private void ActivityFeedItem_Click(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is ActivityEntry entry)
            {
                OpenActivityEntry(entry);
            }
            else if ((sender as FrameworkElement)?.DataContext is HistoryItem item)
            {
                if (item.TypeIcon == "Ã°Å¸â€œÅ½")
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
            if ((sender as FrameworkElement)?.DataContext is ActivityEntry entry)
            {
                OpenActivityEntry(entry);
            }
            else
            {
                ActivityFeedItem_Click(sender, e);
            }
        }

        private void ApplyPendingClipboard(string contentHash)
        {
            System.Threading.Tasks.Task.Run(() =>
            {
                try
                {
                    var response = DaemonClient.ApplyClipboard(contentHash);
                    DeskdropStore.Shared.UpdateStateFromDaemon();
                    Dispatcher.Invoke(() =>
                    {
                        ShowToast(response == null ? "Deskdrop engine is unreachable." : "Clipboard applied.", response == null);
                    });
                }
                catch (Exception ex)
                {
                    Dispatcher.Invoke(() => ShowToast($"Could not apply clipboard: {ex.Message}", true));
                }
            });
        }

        private void OpenActivityEntry(ActivityEntry entry)
        {
            if (entry.CanApply && !string.IsNullOrWhiteSpace(entry.content_hash))
            {
                ApplyPendingClipboard(entry.content_hash);
                return;
            }

            if (!string.IsNullOrWhiteSpace(entry.dest_path))
            {
                RevealPath(entry.dest_path);
                return;
            }

            var text = !string.IsNullOrWhiteSpace(entry.text_preview) ? entry.text_preview : entry.summary;
            if (!string.IsNullOrWhiteSpace(text))
            {
                System.Windows.Clipboard.SetText(text);
                ShowToast("Copied to clipboard.");
            }
        }

        private void BtnPauseSync_Click(object sender, RoutedEventArgs e)
        {
            System.Threading.Tasks.Task.Run(() =>
            {
                DaemonClient.SetSyncEnabled(false);
                DeskdropStore.Shared.UpdateStateFromDaemon();
                Dispatcher.Invoke(() => ShowToast("Sync paused."));
            });
        }

        private void BtnResumeSync_Click(object sender, RoutedEventArgs e)
        {
            System.Threading.Tasks.Task.Run(() =>
            {
                DaemonClient.SetSyncEnabled(true);
                DeskdropStore.Shared.UpdateStateFromDaemon();
                Dispatcher.Invoke(() => ShowToast("Sync resumed."));
            });
        }

        private void BtnScanNow_Click(object sender, RoutedEventArgs e)
        {
            System.Threading.Tasks.Task.Run(() =>
            {
                DaemonClient.RescanPeers();
                Dispatcher.Invoke(() => ShowToast("Scanning for peers..."));
            });
        }

        private void BtnStopService_Click(object sender, RoutedEventArgs e)
        {
            var result = MessageBox.Show(this, "Are you sure you want to stop the Deskdrop service? The app will close.", "Stop Service", MessageBoxButton.YesNo, MessageBoxImage.Warning);
            if (result == MessageBoxResult.Yes)
            {
                System.Threading.Tasks.Task.Run(() =>
                {
                    DaemonClient.Shutdown();
                    Dispatcher.Invoke(() => System.Windows.Application.Current.Shutdown());
                });
            }
        }

        private void Launchpad_MagicLinkPair_Click(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            var qrWindow = new QRPairingWindow();
            qrWindow.Owner = this;
            qrWindow.ShowDialog();
        }

        private void BtnPairDevice_Click(object sender, RoutedEventArgs e)
        {
            if (SelectedPeer != null)
            {
                System.Threading.Tasks.Task.Run(() =>
                {
                    try
                    {
                        DaemonClient.SendPairingRequest(SelectedPeer.device_id);
                        DeskdropStore.Shared.UpdateStateFromDaemon();
                    }
                    catch (Exception ex)
                    {
                        Dispatcher.Invoke(() => ShowToast($"Failed to send pairing request: {ex.Message}", true));
                    }
                });
            }
        }

        private void BtnAcceptPairing_Click(object sender, RoutedEventArgs e)
        {
            if (SelectedPeer != null)
            {
                System.Threading.Tasks.Task.Run(() =>
                {
                    try
                    {
                        DaemonClient.RespondToPairing(SelectedPeer.device_id, true);
                        DeskdropStore.Shared.UpdateStateFromDaemon();
                    }
                    catch (Exception ex)
                    {
                        Dispatcher.Invoke(() => ShowToast($"Failed to accept pairing: {ex.Message}", true));
                    }
                });
            }
        }

        private void BtnDeclinePairing_Click(object sender, RoutedEventArgs e)
        {
            if (SelectedPeer != null)
            {
                System.Threading.Tasks.Task.Run(() =>
                {
                    try
                    {
                        DaemonClient.RespondToPairing(SelectedPeer.device_id, false);
                        DeskdropStore.Shared.UpdateStateFromDaemon();
                    }
                    catch (Exception ex)
                    {
                        Dispatcher.Invoke(() => ShowToast($"Failed to decline pairing: {ex.Message}", true));
                    }
                });
            }
        }

        private static void RevealPath(string path)
        {
            if (System.IO.File.Exists(path))
            {
                System.Diagnostics.Process.Start("explorer.exe", $"/select,\"{path}\"");
            }
            else if (System.IO.Directory.Exists(path))
            {
                System.Diagnostics.Process.Start("explorer.exe", $"\"{path}\"");
            }
        }
            private void BtnRemoteExplorer_Click(object sender, RoutedEventArgs e)
        {
            if (SelectedPeer != null)
            {
                var explorer = new RemoteExplorerWindow(SelectedPeer.device_id, SelectedPeer.DisplayName);
                explorer.Owner = this;
                explorer.Show();
            }
            else
            {
                ShowToast("Please select a device first.");
            }
        }

        private void Launchpad_TransferFiles_Click(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            if (SelectedPeer == null)
            {
                SelectedPeer = System.Linq.Enumerable.FirstOrDefault(DeskdropStore.Shared.ConnectedPeers);
            }
            if (SelectedPeer != null)
            {
                var dialog = new Microsoft.Win32.OpenFileDialog { Multiselect = true, Title = $"Select files to send to {SelectedPeer.DisplayName}" };
                if (dialog.ShowDialog() == true)
                {
                    foreach (var file in dialog.FileNames)
                    {
                        DaemonClient.RemoteFileActionRequest(SelectedPeer.device_id, "upload", file);
                    }
                }
            }
        }

        private void Launchpad_BrowseDevice_Click(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            if (SelectedPeer == null)
            {
                SelectedPeer = System.Linq.Enumerable.FirstOrDefault(DeskdropStore.Shared.ConnectedPeers);
            }
            if (SelectedPeer != null)
            {
                var remoteWin = new RemoteExplorerWindow(SelectedPeer.device_id, SelectedPeer.DisplayName);
                remoteWin.Owner = this;
                remoteWin.Show();
            }
        }
    }
}
