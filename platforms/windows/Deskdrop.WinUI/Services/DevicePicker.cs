using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Deskdrop.WinUI.Services
{
    // Several "quick action" paths (push clipboard, quick send, title-bar
    // send) used to resolve their target via ConnectedPeers.FirstOrDefault()
    // - invisible with one connected device, but silently acts on an
    // arbitrary one once a second is connected, with no indication which.
    // This prompts instead whenever there's real ambiguity.
    public static class DevicePicker
    {
        public static async Task<PeerViewModel?> PickAsync(XamlRoot? xamlRoot, IEnumerable<PeerViewModel> connectedPeers)
        {
            var peers = connectedPeers.ToList();
            if (peers.Count == 0) return null;
            if (peers.Count == 1) return peers[0];
            if (xamlRoot == null) return peers[0];

            var choice = await ShowAsync(xamlRoot, peers, offerAll: false);
            return choice?.Peer;
        }

        // File sends can also go to every connected device at once - a null
        // target in send_file_path, same as Android and macOS. Returns null
        // when the user cancels; otherwise the chosen target, where a null
        // DeviceId means all connected devices.
        public static async Task<SendTarget?> PickSendTargetAsync(XamlRoot? xamlRoot, IEnumerable<PeerViewModel> connectedPeers)
        {
            var peers = connectedPeers.ToList();
            if (peers.Count == 0) return new SendTarget(null);
            if (peers.Count == 1 || xamlRoot == null) return new SendTarget(peers[0].device_id);

            var choice = await ShowAsync(xamlRoot, peers, offerAll: true);
            if (choice == null) return null;
            return new SendTarget(choice.Value.All ? null : choice.Value.Peer?.device_id);
        }

        private static async Task<(PeerViewModel? Peer, bool All)?> ShowAsync(XamlRoot xamlRoot, List<PeerViewModel> peers, bool offerAll)
        {
            // Show the same identity people see on the Devices page - a
            // device glyph, its name, and its platform - rather than a bare
            // string list. Choosing a target is a recognition task, and
            // recognition needs the icon.
            var listView = new ListView
            {
                ItemsSource = peers,
                SelectionMode = ListViewSelectionMode.Single,
                SelectedIndex = 0,
                MinWidth = 300,
            };

            // Prefer the richer row, but never let a template problem stop
            // someone from sending a file - fall back to plain names.
            var template = BuildPeerTemplate();
            if (template != null) listView.ItemTemplate = template;
            else listView.DisplayMemberPath = nameof(PeerViewModel.DisplayName);

            CheckBox? allBox = null;
            object content = listView;
            if (offerAll)
            {
                allBox = new CheckBox
                {
                    Content = $"All connected devices ({peers.Count})",
                    Margin = new Thickness(4, 8, 0, 0),
                };
                allBox.Checked += (_, _) => listView.IsEnabled = false;
                allBox.Unchecked += (_, _) => listView.IsEnabled = true;
                var panel = new StackPanel();
                panel.Children.Add(listView);
                panel.Children.Add(allBox);
                content = panel;
            }

            var dialog = new ContentDialog
            {
                Title = "Send to which device?",
                Content = content,
                PrimaryButtonText = "Send",
                CloseButtonText = "Cancel",
                DefaultButton = ContentDialogButton.Primary,
                XamlRoot = xamlRoot,
            };

            var result = await dialog.ShowAsync();
            if (result != ContentDialogResult.Primary) return null;
            bool all = allBox?.IsChecked == true;
            var peer = listView.SelectedItem as PeerViewModel;
            if (!all && peer == null) return null;
            return (peer, all);
        }

        // Built in code rather than XAML because this picker is raised from
        // several pages and has no view of its own to host a resource.
        private static DataTemplate? BuildPeerTemplate()
        {
            const string markup = """
                <DataTemplate xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
                              xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
                    <Grid ColumnSpacing="12" Padding="0,4">
                        <Grid.ColumnDefinitions>
                            <ColumnDefinition Width="Auto" />
                            <ColumnDefinition Width="*" />
                        </Grid.ColumnDefinitions>
                        <Border Grid.Column="0" Width="30" Height="30"
                                CornerRadius="6"
                                Background="{ThemeResource AppSurfaceSubtleBrush}">
                            <FontIcon Glyph="&#xE8EA;" FontSize="14"
                                      Foreground="{ThemeResource TextFillColorSecondaryBrush}"
                                      HorizontalAlignment="Center" VerticalAlignment="Center" />
                        </Border>
                        <StackPanel Grid.Column="1" VerticalAlignment="Center">
                            <TextBlock Text="{Binding DisplayName}" FontSize="13" FontWeight="SemiBold" />
                            <TextBlock Text="{Binding PlatformLabel}" FontSize="11.5"
                                       Foreground="{ThemeResource TextFillColorTertiaryBrush}" />
                        </StackPanel>
                    </Grid>
                </DataTemplate>
                """;

            try
            {
                return Microsoft.UI.Xaml.Markup.XamlReader.Load(markup) as DataTemplate;
            }
            catch (System.Exception ex)
            {
                App.HandleError(ex);
                return null;
            }
        }
    }

    public sealed record SendTarget(string? DeviceId);
}
