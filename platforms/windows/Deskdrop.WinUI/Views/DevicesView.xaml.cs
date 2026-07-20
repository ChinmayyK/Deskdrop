using Microsoft.UI.Xaml.Controls;
using System.Linq;

namespace Deskdrop.WinUI.Views
{
    public sealed partial class DevicesView : Page
    {
        private DeskdropStore _mgr;

        public DevicesView()
        {
            this.InitializeComponent();
            
            DeskdropStore.Shared.PropertyChanged += (s, e) => {
                if (e.PropertyName == nameof(DeskdropStore.Shared.Peers))
                {
                    DispatcherQueue.TryEnqueue(() => {
                        DeviceTargetsList.ItemsSource = DeskdropStore.Shared.Peers.ToList();
                    });
                }
            };

            DeviceTargetsList.ItemsSource = DeskdropStore.Shared.Peers.ToList();
        }
    }
}
