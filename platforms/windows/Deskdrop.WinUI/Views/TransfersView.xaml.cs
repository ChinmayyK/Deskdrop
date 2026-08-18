using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

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

        private void OnCancelClicked(object sender, RoutedEventArgs e)
        {
            if ((sender as FrameworkElement)?.DataContext is FileTransferState transfer)
            {
                DaemonClient.CancelFileTransfer(transfer.transfer_id);
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
                    var firstConnected = mgr.ConnectedPeers.FirstOrDefault();
                    foreach (var file in files)
                    {
                        DaemonClient.SendFilePath(firstConnected?.device_id, file.Path, file.Name, file.ContentType);
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
