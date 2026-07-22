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
    }
}
