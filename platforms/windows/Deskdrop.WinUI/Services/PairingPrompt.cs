using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Deskdrop.WinUI.Services
{
    // A pairing request used to surface only as a card on the Devices page
    // and a plain toast with no actions - so anyone on another page, or with
    // the window in the tray, never saw it, and the other device just sat on
    // "waiting". Mac and Android put the request in front of the user with
    // the security code and an explicit Accept / Decline; this is the same
    // moment for Windows. The Devices-page card stays as the fallback when
    // the dialog can't be shown (another dialog is already open).
    public static class PairingPrompt
    {
        private static readonly HashSet<string> _prompted = new();
        private static bool _isOpen;

        // Called on the UI thread each time the store re-syncs its
        // PairingRequests projection.
        public static void Sync(IReadOnlyCollection<PeerViewModel> requests)
        {
            // Forget requests that were answered or withdrawn, so the same
            // device asking again later is prompted again.
            _prompted.IntersectWith(requests.Select(p => p.device_id));
            if (_isOpen) return;

            var next = requests.FirstOrDefault(p => !_prompted.Contains(p.device_id));
            if (next == null) return;

            var root = DashboardWindow.Current?.Content?.XamlRoot;
            if (root == null) return; // tray-only: the actionable toast covers it

            _prompted.Add(next.device_id);
            _ = ShowAsync(next, root);
        }

        private static async Task ShowAsync(PeerViewModel peer, XamlRoot root)
        {
            _isOpen = true;
            try
            {
                var dialog = new ContentDialog
                {
                    Title = $"{peer.DisplayName} wants to pair",
                    Content = BuildContent(peer),
                    PrimaryButtonText = "Accept",
                    SecondaryButtonText = "Decline",
                    CloseButtonText = "Later",
                    DefaultButton = ContentDialogButton.Primary,
                    XamlRoot = root,
                };

                var result = await dialog.ShowAsync();
                if (result == ContentDialogResult.Primary)
                    DeskdropStore.Shared.RespondToPairing(peer.device_id, true);
                else if (result == ContentDialogResult.Secondary)
                    DeskdropStore.Shared.RespondToPairing(peer.device_id, false);
            }
            catch (Exception ex)
            {
                // Most often "only one ContentDialog can be open at a time".
                // Let it be prompted again on the next sync.
                _prompted.Remove(peer.device_id);
                TraceLog.Write($"PairingPrompt: could not show dialog - {ex.Message}");
            }
            finally
            {
                _isOpen = false;
            }
        }

        private static UIElement BuildContent(PeerViewModel peer)
        {
            var pin = new TextBlock
            {
                FontFamily = new FontFamily("Cascadia Mono, Consolas"),
                FontSize = 30,
                FontWeight = FontWeights.SemiBold,
                CharacterSpacing = 160,
                HorizontalAlignment = HorizontalAlignment.Center,
            };
            // Bound rather than copied: the code can land a poll after the
            // request itself, and a blank code must fill in, not stay blank.
            pin.SetBinding(TextBlock.TextProperty, new Microsoft.UI.Xaml.Data.Binding
            {
                Source = peer,
                Path = new PropertyPath(nameof(PeerViewModel.pairingPin)),
                FallbackValue = "--- ---",
                TargetNullValue = "--- ---",
            });

            return new StackPanel
            {
                Spacing = 14,
                MinWidth = 320,
                Children =
                {
                    new TextBlock
                    {
                        Text = $"Check that this code matches the one shown on {peer.DisplayName}. Only accept if it does.",
                        TextWrapping = TextWrapping.Wrap,
                    },
                    new Border
                    {
                        Padding = new Thickness(16, 12, 16, 12),
                        CornerRadius = new CornerRadius(10),
                        Background = Application.Current.Resources.TryGetValue("AppSurfaceSubtleBrush", out var bg) && bg is Brush b
                            ? b
                            : new SolidColorBrush(Microsoft.UI.Colors.Transparent),
                        Child = pin,
                    },
                    new TextBlock
                    {
                        Text = "Once paired, the two devices reconnect automatically and share clipboard and files.",
                        TextWrapping = TextWrapping.Wrap,
                        Opacity = 0.7,
                        FontSize = 12,
                    },
                },
            };
        }
    }
}
