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

            var dialog = new ContentDialog
            {
                Title = "Send to which device?",
                Content = listView,
                PrimaryButtonText = "Send",
                CloseButtonText = "Cancel",
                DefaultButton = ContentDialogButton.Primary,
                XamlRoot = xamlRoot,
            };

            var result = await dialog.ShowAsync();
            if (result != ContentDialogResult.Primary) return null;
            return listView.SelectedItem as PeerViewModel;
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
}
