using System;
using System.IO;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using Hardcodet.Wpf.TaskbarNotification;
using Microsoft.Win32;

namespace Deskdrop.Windows
{
    internal sealed class TrayApp
    {
        private readonly TaskbarIcon _tray;
        private readonly ClipboardManager _mgr = new();
        private readonly ContextMenu _menu = new();

        private readonly MenuItem _statusItem;
        private readonly MenuItem _sendItem;
        private readonly MenuItem _sendFileItem;
        private readonly MenuItem _syncToggleItem;

        private MainWindow? _mainWindow;
        private QuickAccessWindow? _quickAccessWindow;
        private DropZoneWindow? _dropZoneWindow;
        private bool _syncEnabled = true;
        private DateTime _lastBalloonAt = DateTime.MinValue;

        public TrayApp()
        {
            _statusItem = new MenuItem { Header = "Starting…", IsEnabled = false };

            _sendItem = new MenuItem { Header = "Send Clipboard to Devices", IsEnabled = false };
            _sendItem.Click += OnSendClipboard;

            _sendFileItem = new MenuItem { Header = "Send File to Devices…", IsEnabled = false };
            _sendFileItem.Click += OnSendFile;

            _syncToggleItem = new MenuItem { Header = "Pause Sync" };
            _syncToggleItem.Click += OnToggleSync;

            var historyItem = new MenuItem { Header = "Open Dashboard…" };
            historyItem.Click += (_, _) => OpenDashboard();

            var scanItem = new MenuItem { Header = "Scan for Devices" };
            scanItem.Click += OnScanDevices;

            var connectItem = new MenuItem { Header = "Connect to Device…" };
            connectItem.Click += OnManualConnect;

            var prefsItem = new MenuItem { Header = "Preferences…" };
            prefsItem.Click += (_, _) => OpenDashboard();

            var quitItem = new MenuItem { Header = "Quit Deskdrop" };
            quitItem.Click += (_, _) => { 
                _mgr.Stop(); 
                System.Windows.Application.Current?.Shutdown();
            };

            Microsoft.Win32.SystemEvents.PowerModeChanged += OnPowerModeChanged;
            System.Net.NetworkInformation.NetworkChange.NetworkAddressChanged += OnNetworkAddressChanged;

            _menu.Items.Add(_statusItem);
            _menu.Items.Add(new Separator());
            _menu.Items.Add(_sendItem);
            _menu.Items.Add(_sendFileItem);
            _menu.Items.Add(historyItem);
            _menu.Items.Add(new Separator());
            _menu.Items.Add(_syncToggleItem);
            _menu.Items.Add(scanItem);
            _menu.Items.Add(connectItem);
            _menu.Items.Add(new Separator());
            _menu.Items.Add(prefsItem);
            _menu.Items.Add(new Separator());
            _menu.Items.Add(quitItem);

            _tray = new TaskbarIcon
            {
                IconSource = BuildTrayIcon(false),
                ToolTipText = "Deskdrop",
                ContextMenu = _menu
            };
            
            _tray.TrayMouseDoubleClick += (_, _) => OpenDashboard();
            _tray.TrayLeftMouseUp += (_, _) => OpenQuickAccess();

            // Register Global Hotkeys
            if (Program.LoadSettings().EnableHotkeys)
            {
                GlobalHotKeyManager.Shared.Register(System.Windows.Input.ModifierKeys.Control | System.Windows.Input.ModifierKeys.Shift, System.Windows.Input.Key.V, () => {
                    System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                        OpenQuickAccess();
                    });
                });

                GlobalHotKeyManager.Shared.Register(System.Windows.Input.ModifierKeys.Control, System.Windows.Input.Key.K, () => {
                    System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                        OpenDashboard();
                        if (_mainWindow != null)
                        {
                            _mainWindow.ToggleCommandPaletteGlobal();
                        }
                    });
                });

                GlobalHotKeyManager.Shared.Register(System.Windows.Input.ModifierKeys.Control | System.Windows.Input.ModifierKeys.Shift, System.Windows.Input.Key.L, () => {
                    System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                        Task.Run(async () => {
                            var url = await BrowserUrlFetcher.GetActiveBrowserUrl();
                            if (!string.IsNullOrEmpty(url))
                            {
                                _mgr.PushText(url);
                                System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                                    NotificationHelper.ShowToast("Deskdrop", $"Pushed URL: {url}");
                                });
                            }
                        });
                    });
                });

                GlobalHotKeyManager.Shared.Register(System.Windows.Input.ModifierKeys.Control, System.Windows.Input.Key.D, () => {
                    System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                        ToggleDropCanvas();
                    });
                });

                GlobalHotKeyManager.Shared.Register(System.Windows.Input.ModifierKeys.Control | System.Windows.Input.ModifierKeys.Shift, System.Windows.Input.Key.C, () => {
                    System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                        if (System.Windows.Clipboard.ContainsText() || System.Windows.Clipboard.ContainsImage() || System.Windows.Clipboard.ContainsFileDropList())
                        {
                            _mgr.PushLocalClipboard();
                            NotificationHelper.ShowToast("Deskdrop", "Clipboard sent.");
                        }
                    });
                });
            }

            _mgr.StatusChanged       += OnStatusChanged;
            _mgr.TofuPromptRequested += OnTofuPrompt;
            _mgr.HistoryItemAdded    += item => {
                if (!Program.LoadSettings().ShowNotifications) return;
                System.Windows.Application.Current?.Dispatcher.Invoke(() =>
                {
                    string title = item.Source == "local" ? "Sent Clipboard" : $"Received from {item.Source}";
                    NotificationHelper.ShowToast(title, $"{item.TypeIcon} {item.Summary}");
                });
            };

            _mgr.IncomingCallRequested += (caller, deviceId, state) => {
                System.Windows.Application.Current?.Dispatcher.Invoke(() =>
                {
                    if (state == "ringing")
                    {
                        var banner = new IncomingCallBannerWindow(caller);
                        banner.CallAccepted += (s, e) => _mgr.SendCallAction("accept", deviceId);
                        banner.CallDeclined += (s, e) => _mgr.SendCallAction("reject", deviceId);
                        banner.Show();
                    }
                    else
                    {
                        foreach (System.Windows.Window window in System.Windows.Application.Current.Windows)
                        {
                            if (window is IncomingCallBannerWindow bannerWindow)
                            {
                                bannerWindow.Close();
                            }
                        }
                    }
                });
            };

            var s = Program.LoadSettings();
            _syncEnabled = s.SyncEnabled;
            _syncToggleItem.Header = _syncEnabled ? "Pause Sync" : "Resume Sync";
            _mgr.Start(
                deviceName: string.IsNullOrWhiteSpace(s.DeviceName) ? Environment.MachineName : s.DeviceName,
                port: s.Port);
        }

        private void OnStatusChanged(string msg)
        {
            if (_tray == null) return;
            System.Windows.Application.Current?.Dispatcher.Invoke(() =>
            {
                _statusItem.Header = msg.Length > 63 ? msg[..60] + "…" : msg;
                RefreshTrayState();
            });
        }

        public void RefreshTrayState()
        {
            if (_tray == null) return;
            System.Windows.Application.Current?.Dispatcher.Invoke(() =>
            {
                bool connected = _mgr.IsConnected();
                _tray.IconSource = BuildTrayIcon(connected);
                _tray.ToolTipText = connected ? "Deskdrop — syncing" : "Deskdrop — idle";
                _sendItem.IsEnabled = connected;
                _sendFileItem.IsEnabled = connected;
            });
        }

        private void OnClipboardReceived(string text, string from)
        {
            if (!Program.LoadSettings().ShowNotifications) return;
            if ((DateTime.Now - _lastBalloonAt).TotalSeconds < 3) return;
            _lastBalloonAt = DateTime.Now;
            string preview = text.Length > 60 ? text[..57] + "…" : text;
            System.Windows.Application.Current?.Dispatcher.Invoke(() =>
                NotificationHelper.ShowToast($"📋 Clipboard from {from}", preview));
        }

        private void OnTofuPrompt(string deviceId, string deviceName, string fingerprint)
        {
            System.Windows.Application.Current?.Dispatcher.Invoke(() =>
            {
                OpenDashboard();
                if (_mainWindow != null)
                {
                    _mainWindow.ShowTofuPrompt(deviceId, deviceName, fingerprint);
                }
            });
        }

        private void OnSendClipboard(object? s, RoutedEventArgs e)
        {
            Task.Run(() =>
            {
                System.Windows.Application.Current?.Dispatcher.Invoke(() => {
                    if (System.Windows.Clipboard.ContainsText() || System.Windows.Clipboard.ContainsImage() || System.Windows.Clipboard.ContainsFileDropList())
                    {
                        _mgr.PushLocalClipboard();
                        NotificationHelper.ShowToast("Deskdrop", "Clipboard sent.");
                    }
                });
            });
        }

        private void OnSendFile(object? s, RoutedEventArgs e)
        {
            var ofd = new Microsoft.Win32.OpenFileDialog { Title = "Select file to send via Deskdrop" };
            if (ofd.ShowDialog() == true)
            {
                Task.Run(() => {
                    try {
                        _mgr.PushFile(ofd.FileName);
                        System.Windows.Application.Current?.Dispatcher.Invoke(() =>
                            NotificationHelper.ShowToast("Deskdrop", $"Sending {Path.GetFileName(ofd.FileName)}..."));
                    } catch (Exception ex) {
                        System.Windows.Application.Current?.Dispatcher.Invoke(() =>
                            NotificationHelper.ShowToast("Deskdrop Error", $"Failed to send file: {ex.Message}"));
                    }
                });
            }
        }

        private void OnToggleSync(object? s, RoutedEventArgs e)
        {
            _syncEnabled = !_syncEnabled;
            _syncToggleItem.Header = _syncEnabled ? "Pause Sync" : "Resume Sync";
            using var k = Registry.CurrentUser.CreateSubKey(@"Software\Deskdrop");
            k.SetValue("SyncEnabled", _syncEnabled ? 1 : 0, RegistryValueKind.DWord);
            Task.Run(() => DaemonClient.Send(new { cmd = "save_settings", sync_enabled = _syncEnabled }));
            NotificationHelper.ShowToast("Deskdrop",
                _syncEnabled ? "Clipboard sync resumed." : "Clipboard sync paused.");
        }

        private void OnManualConnect(object? s, RoutedEventArgs e)
        {
            System.Windows.Application.Current?.Dispatcher.Invoke(() =>
            {
                OpenDashboard();
            });
        }

        private void OnScanDevices(object? s, RoutedEventArgs e)
        {
            Task.Run(() => DaemonClient.Send(new { cmd = "rescan_peers" }));
            NotificationHelper.ShowToast("Deskdrop", "Scanning for nearby devices…");
        }

        public void PushFileExternal(string filePath)
        {
            try {
                _mgr.PushFile(filePath);
                System.Windows.Application.Current?.Dispatcher.Invoke(() =>
                    NotificationHelper.ShowToast("Deskdrop", $"Sending {Path.GetFileName(filePath)}..."));
            } catch (Exception ex) {
                System.Windows.Application.Current?.Dispatcher.Invoke(() =>
                    NotificationHelper.ShowToast("Deskdrop Error", $"Failed to send file: {ex.Message}"));
            }
        }
        
        public void PushClipboardExternal()
        {
            OnSendClipboard(this, new RoutedEventArgs());
        }

        public void OpenSendFileDialog()
        {
            OnSendFile(this, new RoutedEventArgs());
        }

        public void RespondToTrustExternal(string deviceId, bool accepted)
        {
            _mgr.RespondToTrust(deviceId, accepted);
            if (accepted)
            {
                System.Windows.Application.Current?.Dispatcher.Invoke(() =>
                    NotificationHelper.ShowToast("Deskdrop", "Device trusted successfully."));
            }
        }

        public void OpenDashboard()
        {
            System.Windows.Application.Current?.Dispatcher.Invoke(() =>
            {
                try
                {
                    if (_mainWindow == null)
                    {
                        _mainWindow = new MainWindow(_mgr);
                        _mainWindow.Closed += (_, _) => _mainWindow = null;
                    }
                    
                    _mainWindow.Show();
                    if (_mainWindow.WindowState == System.Windows.WindowState.Minimized)
                    {
                        _mainWindow.WindowState = System.Windows.WindowState.Normal;
                    }
                    _mainWindow.Activate();
                    _mainWindow.Topmost = true;
                    _mainWindow.Topmost = false;
                    _mainWindow.Focus();
                }
                catch (Exception ex)
                {
                    _mainWindow = null;
                    Program.LogError(ex);
                }
            });
        }

        public void OpenQuickAccess()
        {
            if (_quickAccessWindow != null && _quickAccessWindow.IsLoaded)
            {
                _quickAccessWindow.Activate();
                return;
            }

            _quickAccessWindow = new QuickAccessWindow(_mgr);
            _quickAccessWindow.DashboardRequested += (s, e) => OpenDashboard();
            _quickAccessWindow.Show();
            _quickAccessWindow.Activate();
        }

        // MARK: - Tray Icon (Original Backup for Rollback)
        private static ImageSource BuildTrayIconOriginal(bool connected)
        {
            try {
                return new BitmapImage(new Uri("pack://application:,,,/Assets/logo.png"));
            } catch {
                return null!;
            }
        }

        private static ImageSource BuildTrayIcon(bool connected)
        {
            try
            {
                // Experimental: Connected Devices icon (Laptop + Smartphone vector glyph matching macOS)
                int size = 16;
                var visual = new DrawingVisual();
                using (var dc = visual.RenderOpen())
                {
                    var brush = connected ? Brushes.DodgerBlue : Brushes.White;
                    var pen = new Pen(brush, 1.2) { StartLineCap = PenLineCap.Round, EndLineCap = PenLineCap.Round };

                    // Laptop monitor (left side)
                    dc.DrawRoundedRectangle(null, pen, new Rect(1, 3, 9, 7), 1, 1);
                    // Laptop keyboard base
                    dc.DrawLine(pen, new Point(0, 11), new Point(11, 11));

                    // Smartphone (overlapping on right side)
                    dc.DrawRoundedRectangle(brush, pen, new Rect(10, 5, 4, 7), 1, 1);
                }

                var rtb = new RenderTargetBitmap(size, size, 96, 96, PixelFormats.Pbgra32);
                rtb.Render(visual);
                rtb.Freeze();
                return rtb;
            }
            catch
            {
                return BuildTrayIconOriginal(connected);
            }
        }

        private void OnPowerModeChanged(object sender, Microsoft.Win32.PowerModeChangedEventArgs e)
        {
            if (e.Mode == Microsoft.Win32.PowerModes.Resume)
            {
                System.Windows.Application.Current?.Dispatcher.InvokeAsync(() => {
                    _mgr.Stop();
                    var s = Program.LoadSettings();
                    _mgr.Start(
                        deviceName: string.IsNullOrWhiteSpace(s.DeviceName) ? Environment.MachineName : s.DeviceName,
                        port: s.Port);
                });
            }
        }

        private void OnNetworkAddressChanged(object? sender, EventArgs e)
        {
            System.Windows.Application.Current?.Dispatcher.InvokeAsync(async () => {
                await Task.Delay(2000); 
                _mgr.Stop();
                var s = Program.LoadSettings();
                _mgr.Start(
                    deviceName: string.IsNullOrWhiteSpace(s.DeviceName) ? Environment.MachineName : s.DeviceName,
                    port: s.Port);
            });
        }

        private void ToggleDropCanvas()
        {
            if (_dropZoneWindow != null && _dropZoneWindow.IsLoaded && _dropZoneWindow.IsVisible)
            {
                _dropZoneWindow.Close();
                _dropZoneWindow = null;
                return;
            }
            if (_dropZoneWindow != null)
            {
                try { _dropZoneWindow.Close(); } catch { }
            }
            _dropZoneWindow = new DropZoneWindow(_mgr);
            _dropZoneWindow.Closed += (s, e) => { _dropZoneWindow = null; };
            _dropZoneWindow.Show();
            _dropZoneWindow.Activate();
        }

        public static void CompleteOnboarding()
        {
            try
            {
                using var key = Microsoft.Win32.Registry.CurrentUser.CreateSubKey(@"Software\Deskdrop");
                if (key != null)
                {
                    key.SetValue("HasCompletedOnboarding", 1, Microsoft.Win32.RegistryValueKind.DWord);
                }
            }
            catch { }
        }
    }
}
