import os

path = r"C:\Users\CHINMAY KUDALKAR\.gemini\antigravity\scratch\Deskdrop\platforms\windows\Deskdrop.Windows\MainWindow.xaml.cs"
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

# Find the start of ApplyPendingClipboard
idx = content.find("private void ApplyPendingClipboard(string contentHash)")

# Find the start of RevealPath
idx_end = content.find("private static void RevealPath(string path)")

if idx != -1 and idx_end != -1:
    before = content[:idx]
    after = content[idx_end:]
    
    methods = """private void ApplyPendingClipboard(string contentHash)
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

        """
    new_content = before + methods + after
    with open(path, 'w', encoding='utf-8') as f:
        f.write(new_content)
    print("Fixed MainWindow.xaml.cs")
else:
    print("Could not find start/end")
