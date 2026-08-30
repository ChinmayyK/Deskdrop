using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using System.Threading.Tasks;

namespace Deskdrop.WinUI.Views
{
    public sealed partial class TransfersView : Page
    {
        public DeskdropStore mgr => DeskdropStore.Shared;

        public TransfersView()
        {
            this.InitializeComponent();
        }

        private void OnAcceptClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is FileTransferState transfer)
            {
                mgr.AcceptTransfer(transfer.transfer_id);
            }
        }

        private void OnRejectClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is FileTransferState transfer)
            {
                mgr.RejectTransfer(transfer.transfer_id);
            }
        }

        private async void OnCancelClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is FileTransferState transfer)
            {
                var resp = await Task.Run(() => DaemonClient.CancelFileTransfer(transfer.transfer_id));
                DaemonActions.ReportIfFailed("Cancel Transfer", resp);
            }
        }

        // "Show" on a finished transfer reveals the file itself, falling back
        // to the containing folder - which is what File Explorer's own
        // "Show in folder" does, and what people expect from a completed
        // download.
        private void OnOpenDestinationClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is not FileTransferState transfer) return;

            try
            {
                var destination = transfer.destination;
                if (!string.IsNullOrWhiteSpace(destination) && System.IO.File.Exists(destination))
                {
                    System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo
                    {
                        FileName = "explorer.exe",
                        Arguments = $"/select,\"{destination}\"",
                        UseShellExecute = true
                    });
                    return;
                }

                var folder = !string.IsNullOrWhiteSpace(destination)
                    ? System.IO.Path.GetDirectoryName(destination)
                    : null;

                if (string.IsNullOrWhiteSpace(folder) || !System.IO.Directory.Exists(folder))
                {
                    folder = System.IO.Path.Combine(
                        Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), "Downloads");
                }

                System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo
                {
                    FileName = folder,
                    UseShellExecute = true
                });
            }
            catch (System.Exception ex)
            {
                App.HandleError(ex);
            }
        }

        private void OnOpenDownloadFolderClicked(object sender, RoutedEventArgs e)
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
            catch (System.Exception ex)
            {
                App.HandleError(ex);
            }
        }

        private async void OnSendFilesClicked(object sender, RoutedEventArgs e)
        {
            try
            {
                var picker = new Windows.Storage.Pickers.FileOpenPicker();
                var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(App.MainWindow);
                WinRT.Interop.InitializeWithWindow.Initialize(picker, hwnd);
                picker.FileTypeFilter.Add("*");
                picker.SuggestedStartLocation = Windows.Storage.Pickers.PickerLocationId.Downloads;
                var files = await picker.PickMultipleFilesAsync();
                if (files != null && files.Count > 0)
                {
                    var target = await Deskdrop.WinUI.Services.DevicePicker.PickAsync(this.XamlRoot, mgr.ConnectedPeers);
                    if (target == null && mgr.ConnectedPeers.Count > 0) return; // user cancelled the picker
                    foreach (var file in files)
                    {
                        // Argument order is (path, name, mime, targetDevice, ...); see the
                        // matching fix note in DashboardWindow.xaml.cs.
                        DaemonClient.SendFilePath(file.Path, file.Name, file.ContentType, target?.device_id);
                    }
                }
            }
            catch (System.Exception ex)
            {
                App.HandleError(ex);
            }
        }
    }
}
