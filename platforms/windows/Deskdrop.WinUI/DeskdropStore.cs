using Microsoft.UI.Dispatching;
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Text.Json;
using System.Text.Json.Serialization;
using Microsoft.UI.Xaml;


namespace Deskdrop.WinUI
{
    public class BaseViewModel : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler? PropertyChanged;
        protected void OnPropertyChanged([CallerMemberName] string? propertyName = null)
        {
            var queue = App.MainDispatcherQueue ?? Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
            if (queue != null && !queue.HasThreadAccess)
            {
                queue.TryEnqueue(() =>
                {
                    PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
                });
            }
            else
            {
                PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
            }
        }
        protected bool SetProperty<T>(ref T backingStore, T value, [CallerMemberName] string propertyName = "", Action? onChanged = null)
        {
            if (EqualityComparer<T>.Default.Equals(backingStore, value)) return false;
            backingStore = value;
            onChanged?.Invoke();
            OnPropertyChanged(propertyName);
            return true;
        }
    }

    public class ActivityEntry : BaseViewModel
    {
        private ulong _id;
        public ulong id { get => _id; set => SetProperty(ref _id, value); }
        private string _kind = "";
        public string kind { get => _kind; set { if (SetProperty(ref _kind, value)) NotifyDisplayProperties(); } }
        private string _summary = "";
        public string summary { get => _summary; set { if (SetProperty(ref _summary, value)) NotifyDisplayProperties(); } }
        private ulong _timestampMs;
        [JsonPropertyName("timestamp_ms")]
        public ulong timestamp_ms { get => _timestampMs; set { if (SetProperty(ref _timestampMs, value)) OnPropertyChanged(nameof(RelativeTime)); } }
        private string _device_id = "";
        public string device_id { get => _device_id; set => SetProperty(ref _device_id, value); }
        private string _device_name = "";
        public string device_name { get => _device_name; set { if (SetProperty(ref _device_name, value)) NotifyDisplayProperties(); } }
        private string? _content_hash;
        public string? content_hash { get => _content_hash; set { if (SetProperty(ref _content_hash, value)) NotifyDisplayProperties(); } }
        private string? _text_preview;
        public string? text_preview { get => _text_preview; set { if (SetProperty(ref _text_preview, value)) NotifyDisplayProperties(); } }
        private string? _file_name;
        public string? file_name { get => _file_name; set { if (SetProperty(ref _file_name, value)) NotifyDisplayProperties(); } }
        private long? _file_bytes;
        public long? file_bytes { get => _file_bytes; set { if (SetProperty(ref _file_bytes, value)) NotifyDisplayProperties(); } }
        private string? _transfer_id;
        public string? transfer_id { get => _transfer_id; set => SetProperty(ref _transfer_id, value); }
        private string? _dest_path;
        public string? dest_path { get => _dest_path; set => SetProperty(ref _dest_path, value); }
        private bool _applied_locally;
        public bool applied_locally { get => _applied_locally; set { if (SetProperty(ref _applied_locally, value)) NotifyDisplayProperties(); } }
        private System.Collections.Generic.List<string> _relay_path = new();
        public System.Collections.Generic.List<string> relay_path { get => _relay_path; set { if (SetProperty(ref _relay_path, value ?? new())) NotifyDisplayProperties(); } }

        // Compatibility for older in-process history bindings. JsonIgnore on
        // both: neither is a wire field (device_name is), and without it
        // System.Text.Json throws "JSON property name ... collides with
        // another property" at type-info build time because the active
        // PropertyNameCaseInsensitive options make "source"/"Source"
        // ambiguous - which was silently breaking EVERY ActivityFeed
        // deserialization (see DeserializeList<ActivityEntry> callers).
        [JsonIgnore]
        public ulong timestamp { get => timestamp_ms; set => timestamp_ms = value; }
        [JsonIgnore]
        public string source { get => device_name; set => device_name = value; }

        public string Title => !string.IsNullOrWhiteSpace(file_name)
            ? file_name!
            : (!string.IsNullOrWhiteSpace(text_preview) ? text_preview! : summary);
        public string Preview => !string.IsNullOrWhiteSpace(text_preview) ? text_preview! : summary;
        [JsonIgnore]
        public string Source => string.IsNullOrWhiteSpace(device_name) ? "Deskdrop" : device_name;
        public string TypeLabel => kind switch
        {
            "remote_clipboard_available" => "Clipboard",
            "clipboard_applied" => "Applied",
            "clipboard_text" => "Text",
            "clipboard_image" => "Image",
            "file_transfer_started" => "File",
            "file_transfer_complete" => "File",
            "file_transfer_failed" => "Failed",
            "peer_connected" => "Connection",
            "peer_disconnected" => "Connection",
            "sync_paused" => "Paused",
            "sync_resumed" => "Sync",
            "remote_notification" => "Notification",
            _ => "Event"
        };
        public string TypeIcon => kind switch
        {
            "remote_clipboard_available" => "Clipboard",
            "clipboard_applied" => "CheckCircle",
            "clipboard_text" => "Clipboard",
            "clipboard_image" => "Image",
            "file_transfer_started" => "FileUp",
            "file_transfer_complete" => "FileCheck",
            "file_transfer_failed" => "FileX",
            "peer_connected" => "Wifi",
            "peer_disconnected" => "WifiOff",
            "sync_paused" => "Pause",
            "sync_resumed" => "Play",
            "remote_notification" => "Bell",
            _ => "Activity"
        };
        // Mid-tone values so a single hex reads on both the light and dark
        // canvas; see the note on PeerViewModel.ConnectionColor.
        public string AccentColor => kind switch
        {
            "remote_clipboard_available" => "#3A66D8",
            "clipboard_applied" => "#2AA971",
            "clipboard_image" => "#6E72CF",
            "file_transfer_complete" => "#2AA971",
            "file_transfer_failed" => "#D6483B",
            "peer_connected" => "#2AA971",
            "peer_disconnected" => "#8A8A90",
            "sync_paused" => "#C9861E",
            "remote_notification" => "#6E72CF",
            _ => "#3A66D8"
        };
        public bool CanApply => kind == "remote_clipboard_available" && !applied_locally && !string.IsNullOrWhiteSpace(content_hash);
        public bool HasPreview => !string.IsNullOrWhiteSpace(text_preview);
        public bool HasDestination => !string.IsNullOrWhiteSpace(dest_path);
        public string FormattedSize => file_bytes.HasValue ? DeskdropFormatting.FormatBytes(file_bytes.Value) : "";
        public string RelayPathDisplay => relay_path.Count == 0 ? "" : string.Join(" -> ", relay_path);
        public string RelativeTime => timestamp_ms == 0 ? "Just now" : DeskdropFormatting.RelativeTimeFromUnixMs(timestamp_ms);

        private void NotifyDisplayProperties()
        {
            OnPropertyChanged(nameof(Title));
            OnPropertyChanged(nameof(Preview));
            OnPropertyChanged(nameof(Source));
            OnPropertyChanged(nameof(TypeLabel));
            OnPropertyChanged(nameof(TypeIcon));
            OnPropertyChanged(nameof(AccentColor));
            OnPropertyChanged(nameof(CanApply));
            OnPropertyChanged(nameof(HasPreview));
            OnPropertyChanged(nameof(HasDestination));
            OnPropertyChanged(nameof(FormattedSize));
            OnPropertyChanged(nameof(RelayPathDisplay));
        }
    }

    public class PendingClipboard : BaseViewModel
    {
        private string? _content_hash;
        public string? content_hash { get => _content_hash; set => SetProperty(ref _content_hash, value); }
        private string _summary = "";
        public string summary { get => _summary; set { if (SetProperty(ref _summary, value)) OnPropertyChanged(nameof(Preview)); } }
        private string _device_name = "";
        public string device_name { get => _device_name; set { if (SetProperty(ref _device_name, value)) OnPropertyChanged(nameof(from_device)); } }
        private string? _text_preview;
        public string? text_preview { get => _text_preview; set { if (SetProperty(ref _text_preview, value)) OnPropertyChanged(nameof(Preview)); } }
        private ulong _timestampMs;
        [JsonPropertyName("timestamp_ms")]
        public ulong timestamp_ms { get => _timestampMs; set { if (SetProperty(ref _timestampMs, value)) OnPropertyChanged(nameof(RelativeTime)); } }
        public string from_device { get => device_name; set => device_name = value; }
        public ulong timestamp { get => timestamp_ms; set => timestamp_ms = value; }
        public string Preview => !string.IsNullOrWhiteSpace(text_preview) ? text_preview! : summary;
        public string RelativeTime => timestamp_ms == 0 ? "Just now" : DeskdropFormatting.RelativeTimeFromUnixMs(timestamp_ms);
    }

    public class PeerViewModel : BaseViewModel
    {
        private string _device_id = "";
        [System.Text.Json.Serialization.JsonPropertyName("id")]
        public string device_id { get => _device_id; set => SetProperty(ref _device_id, value); }
        private string _friendly_name = "";
        public string friendly_name { get => _friendly_name; set { if (SetProperty(ref _friendly_name, value)) OnPropertyChanged(nameof(DisplayName)); } }
        private string? _platform;
        public string? platform { get => _platform; set { if (SetProperty(ref _platform, value)) { OnPropertyChanged(nameof(DeviceIcon)); OnPropertyChanged(nameof(IsCameraCapable)); OnPropertyChanged(nameof(ShowCameraButton)); } } }
        private string _status = "";
        public string status { get => _status; set { if(SetProperty(ref _status, value)) NotifyPeerStateProperties(); } }
        private bool _is_trusted;
        [System.Text.Json.Serialization.JsonPropertyName("trusted")]
        public bool is_trusted { get => _is_trusted; set { if(SetProperty(ref _is_trusted, value)) NotifyPeerStateProperties(); } }
        private bool _remembered;
        public bool remembered { get => _remembered; set { if (SetProperty(ref _remembered, value)) NotifyPeerStateProperties(); } }
        private bool _sync_enabled = true;
        public bool sync_enabled { get => _sync_enabled; set { if (SetProperty(ref _sync_enabled, value)) NotifyPeerStateProperties(); } }
        private bool _remote_sync_enabled = true;
        public bool remote_sync_enabled { get => _remote_sync_enabled; set { if (SetProperty(ref _remote_sync_enabled, value)) NotifyPeerStateProperties(); } }
        private bool _auto_connect;
        public bool auto_connect { get => _auto_connect; set { if (SetProperty(ref _auto_connect, value)) NotifyPeerStateProperties(); } }
        private bool _explicit_disconnect;
        [System.Text.Json.Serialization.JsonPropertyName("explicit_disconnect")]
        public bool explicit_disconnect { get => _explicit_disconnect; set { if(SetProperty(ref _explicit_disconnect, value)) NotifyPeerStateProperties(); } }
        private ulong? _last_seen;
        public ulong? last_seen { get => _last_seen; set { if (SetProperty(ref _last_seen, value)) OnPropertyChanged(nameof(LastSeenText)); } }
        private string? _last_error;
        public string? last_error { get => _last_error; set { if (SetProperty(ref _last_error, value)) OnPropertyChanged(nameof(HasError)); } }
        private List<string> _ips = new();
        public List<string> ips { get => _ips; set { if (SetProperty(ref _ips, value)) OnPropertyChanged(nameof(IpAddressText)); } }
        private string? _fingerprint_display;
        public string? fingerprint_display { get => _fingerprint_display; set { if (SetProperty(ref _fingerprint_display, value)) OnPropertyChanged(nameof(HasFingerprint)); } }
        private ulong? _first_seen;
        public ulong? first_seen { get => _first_seen; set { if (SetProperty(ref _first_seen, value)) OnPropertyChanged(nameof(FirstSeenText)); } }

        public string DisplayName => string.IsNullOrWhiteSpace(friendly_name) ? "Nearby device" : friendly_name;
        public string StatusIcon => status == "connected" ? "CheckCircle" : "Circle";
        public string ConnectionText
        {
            get
            {
                if (pairingRequested) return "Wants to pair";
                if (outgoingPairingWaiting) return "Waiting for response";
                if (status == "connected" && !sync_enabled) return "Connected - sync paused";
                if (status == "connected" && !remote_sync_enabled) return "Connected - paused remotely";
                if (status == "connected") return "Connected";
                if (status == "connecting") return "Reconnecting";
                if (explicit_disconnect) return "Disconnected";
                return is_trusted && remembered && auto_connect ? "Ready to reconnect" : "Offline";
            }
        }
        // Literal hex rather than theme brushes: these are view-model values
        // that also feed the tray/notification paths, so they have to read
        // acceptably on both a light and a dark surface. Mid-tone versions of
        // the design system's status colours satisfy both.
        public string ConnectionColor => pairingRequested || outgoingPairingWaiting ? "#C9861E" : (status == "connected" ? "#2AA971" : "#8A8A90");
        public string TrustText => is_trusted ? "Trusted" : "Pairing required";
        public string TrustColor => is_trusted ? "#2AA971" : "#C9861E";
        public string DeviceIcon => (platform ?? friendly_name).ToLowerInvariant() switch
        {
            var p when p.Contains("windows") => "Monitor",
            var p when p.Contains("mac") => "Laptop",
            var p when p.Contains("linux") => "Server",
            _ => "Smartphone"
        };
        public string LastSeenText => last_seen.HasValue ? $"Seen {DeskdropFormatting.RelativeTimeFromUnixSeconds(last_seen.Value)}" : "";
        public string IpAddressText => ips.Count > 0 ? string.Join(", ", ips) : "";
        public bool HasFingerprint => !string.IsNullOrEmpty(fingerprint_display);
        public string FirstSeenText => first_seen.HasValue
            ? DateTimeOffset.FromUnixTimeSeconds((long)first_seen.Value).ToLocalTime().ToString("MMM d, yyyy")
            : "";
        // Gated on connection state, not just "is last_error non-empty": the
        // daemon doesn't clear last_error the moment a device reconnects, it
        // just stops updating it - so a device that reconnected fine kept
        // showing "Couldn't reach" from its last failed attempt forever.
        // Once we're actually connected, any stale error is moot.
        public bool HasError => !IsConnected && !string.IsNullOrWhiteSpace(last_error);
        public bool IsConnected => status == "connected";
        
        public bool ShowVerifyButton => !is_trusted;
        public bool ShowDisconnectButton => status == "connected";
        public bool ShowConnectButton => status != "connected" && is_trusted;
        public bool ShowForgetButton => true;

        // ---- Presentation state for the redesigned device row ----------
        //
        // The row communicates one of four states at a glance: transferring,
        // negotiating (connecting / awaiting a pairing answer), connected, or
        // offline. These predicates are what the card's indicator, action
        // cluster and dimming all key off, so the states stay mutually
        // consistent instead of each control deciding for itself.

        // "In negotiation" - the only state that earns an animated
        // indicator. A steady connection gets a steady dot.
        public bool IsNegotiating => status == "connecting" || pairingRequested || outgoingPairingWaiting;

        public bool IsOffline => status != "connected" && !IsNegotiating;

        // A device we've paired with before, versus one merely seen on the
        // network. Drives the split between "Your devices" and "Nearby".
        public bool IsKnown => is_trusted || remembered;
        public bool IsNearby => !IsKnown;

        // Pairing is the primary action for an unknown device; connecting is
        // the primary action for a known one that's offline.
        public bool ShowPairButton => !is_trusted && !pairingRequested;

        private bool _isTransferring;
        [JsonIgnore]
        public bool IsTransferring
        {
            get => _isTransferring;
            set { if (SetProperty(ref _isTransferring, value)) OnPropertyChanged(nameof(ShowTransferIndicator)); }
        }
        public bool ShowTransferIndicator => IsTransferring && IsConnected;

        public string PlatformLabel
        {
            get
            {
                var p = (platform ?? "").ToLowerInvariant();
                if (p.Contains("windows")) return "Windows PC";
                if (p.Contains("mac") || p.Contains("apple")) return "Mac";
                if (p.Contains("linux")) return "Linux";
                if (p.Contains("android")) return "Android";
                if (p.Contains("ios") || p.Contains("iphone")) return "iPhone";
                return string.IsNullOrWhiteSpace(platform) ? "Device" : platform!;
            }
        }

        public string BatteryPercentText => BatteryLevel > 0 ? $"{BatteryLevel}%" : "";
        public bool HasMetrics => ShowBattery || ShowStorage;

        // Errors are surfaced on the row itself rather than in a toast that
        // scrolls away, and phrased as a state, not a stack trace.
        public string ErrorText => string.IsNullOrWhiteSpace(last_error)
            ? ""
            : $"Couldn't reach {DisplayName}. It may be offline or on another network.";

        // Continuity Camera is phone-to-desktop only: the mobile app is the
        // frame source (deskdrop_push_video_frame), desktop platforms have
        // no camera stream to show. Same platform-string matching as DeviceIcon.
        public bool IsCameraCapable
        {
            get
            {
                var p = (platform ?? friendly_name).ToLowerInvariant();
                return !(p.Contains("windows") || p.Contains("mac") || p.Contains("linux"));
            }
        }
        public bool ShowCameraButton => IsConnected && IsCameraCapable;

        private string? _pairingPin;
        [System.Text.Json.Serialization.JsonPropertyName("pairing_pin")]
        public string? pairingPin { get => _pairingPin; set { if (SetProperty(ref _pairingPin, value)) NotifyPeerStateProperties(); } }
        
        private bool _pairingRequested;
        [System.Text.Json.Serialization.JsonPropertyName("pairing_requested")]
        public bool pairingRequested { get => _pairingRequested; set { if (SetProperty(ref _pairingRequested, value)) NotifyPeerStateProperties(); } }
        private bool _outgoingPairingWaiting;
        [JsonPropertyName("outgoing_pairing_waiting")]
        public bool outgoingPairingWaiting { get => _outgoingPairingWaiting; set { if (SetProperty(ref _outgoingPairingWaiting, value)) NotifyPeerStateProperties(); } }

        private int _batteryLevel;
        public int BatteryLevel { get => _batteryLevel; set { if(SetProperty(ref _batteryLevel, value)) { OnPropertyChanged(nameof(ShowBattery)); OnPropertyChanged(nameof(BatteryIcon)); OnPropertyChanged(nameof(BatteryColor)); } } }
        private bool _batteryCharging;
        public bool BatteryCharging { get => _batteryCharging; set { if(SetProperty(ref _batteryCharging, value)) { OnPropertyChanged(nameof(BatteryIcon)); OnPropertyChanged(nameof(BatteryColor)); } } }
        public bool ShowBattery => BatteryLevel > 0;
        
        public string BatteryIcon
        {
            get
            {
                if (BatteryCharging) return "BatteryCharging"; // Charging icon
                if (BatteryLevel > 80) return "BatteryFull"; // Full
                if (BatteryLevel > 50) return "BatteryMedium"; // Half
                if (BatteryLevel > 20) return "BatteryLow"; // Low
                return "Battery"; // Empty
            }
        }
        public string BatteryColor => BatteryCharging ? "#2AA971" : (BatteryLevel <= 20 ? "#D6483B" : "#8A8A90");

        private long _storageTotal;
        public long StorageTotal { get => _storageTotal; set { if(SetProperty(ref _storageTotal, value)) NotifyStorageProperties(); } }
        private long _storageFree;
        public long StorageFree { get => _storageFree; set { if(SetProperty(ref _storageFree, value)) NotifyStorageProperties(); } }
        private long _storageImages;
        public long StorageImages { get => _storageImages; set { if(SetProperty(ref _storageImages, value)) NotifyStorageProperties(); } }
        private long _storageVideos;
        public long StorageVideos { get => _storageVideos; set { if(SetProperty(ref _storageVideos, value)) NotifyStorageProperties(); } }
        private long _storageApps;
        public long StorageApps { get => _storageApps; set { if(SetProperty(ref _storageApps, value)) NotifyStorageProperties(); } }

        public bool ShowStorage => StorageTotal > 0;
        public string StorageFreeText => StorageTotal > 0 ? $"{DeskdropFormatting.FormatBytes(StorageFree)} free" : "";
        public double StorageImagesRatio => StorageTotal > 0 ? (double)StorageImages / StorageTotal : 0;
        public double StorageVideosRatio => StorageTotal > 0 ? (double)StorageVideos / StorageTotal : 0;
        public double StorageAppsRatio => StorageTotal > 0 ? (double)StorageApps / StorageTotal : 0;
        public double StorageOtherRatio 
        {
            get
            {
                if (StorageTotal == 0) return 0;
                long used = StorageTotal - StorageFree;
                long other = used - StorageImages - StorageVideos - StorageApps;
                if (other < 0) other = 0;
                return (double)other / StorageTotal;
            }
        }

        private void NotifyStorageProperties()
        {
            OnPropertyChanged(nameof(ShowStorage));
            OnPropertyChanged(nameof(StorageFreeText));
            OnPropertyChanged(nameof(StorageImagesRatio));
            OnPropertyChanged(nameof(StorageVideosRatio));
            OnPropertyChanged(nameof(StorageAppsRatio));
            OnPropertyChanged(nameof(StorageOtherRatio));
        }

        private void NotifyPeerStateProperties()
        {
            OnPropertyChanged(nameof(StatusIcon));
            OnPropertyChanged(nameof(ConnectionText));
            OnPropertyChanged(nameof(ConnectionColor));
            OnPropertyChanged(nameof(TrustText));
            OnPropertyChanged(nameof(TrustColor));
            OnPropertyChanged(nameof(IsConnected));
            OnPropertyChanged(nameof(ShowVerifyButton));
            OnPropertyChanged(nameof(ShowDisconnectButton));
            OnPropertyChanged(nameof(ShowConnectButton));
            OnPropertyChanged(nameof(ShowForgetButton));
            OnPropertyChanged(nameof(ShowCameraButton));
            OnPropertyChanged(nameof(IsNegotiating));
            OnPropertyChanged(nameof(IsOffline));
            OnPropertyChanged(nameof(IsKnown));
            OnPropertyChanged(nameof(IsNearby));
            OnPropertyChanged(nameof(ShowPairButton));
            OnPropertyChanged(nameof(ShowTransferIndicator));
            OnPropertyChanged(nameof(HasError));
            OnPropertyChanged(nameof(ErrorText));
        }

        public void NotifyAll()
        {
            NotifyPeerStateProperties();
            NotifyStorageProperties();
            OnPropertyChanged(nameof(DisplayName));
            OnPropertyChanged(nameof(ShowBattery));
            OnPropertyChanged(nameof(BatteryIcon));
            OnPropertyChanged(nameof(BatteryColor));
            OnPropertyChanged(nameof(BatteryPercentText));
            OnPropertyChanged(nameof(HasMetrics));
            OnPropertyChanged(nameof(PlatformLabel));
            OnPropertyChanged(nameof(LastSeenText));
            OnPropertyChanged(nameof(pairingPin));
            OnPropertyChanged(nameof(IpAddressText));
            OnPropertyChanged(nameof(HasFingerprint));
            OnPropertyChanged(nameof(FirstSeenText));
        }
    }

    public class FileTransferState : BaseViewModel
    {
        private string _transfer_id = "";
        public string transfer_id { get => _transfer_id; set => SetProperty(ref _transfer_id, value); }
        private string _from_device = "";
        public string from_device { get => _from_device; set { if (SetProperty(ref _from_device, value)) OnPropertyChanged(nameof(StatusText)); } }
        private string _file_name = "";
        public string file_name { get => _file_name; set { if(SetProperty(ref _file_name, value)) OnPropertyChanged(nameof(FileName)); } }
        private long _bytes_total;
        public long bytes_total { get => _bytes_total; set { if(SetProperty(ref _bytes_total, value)) NotifyProgressProperties(); } }
        private long _bytes_received;
        public long bytes_received { get => _bytes_received; set { if(SetProperty(ref _bytes_received, value)) NotifyProgressProperties(); } }
        private int _percent;
        public int percent { get => _percent; set { if(SetProperty(ref _percent, value)) NotifyProgressProperties(); } }
        private string _status = "";
        public string status { get => _status; set { if(SetProperty(ref _status, value)) NotifyProgressProperties(); } }
        private string? _destination;
        public string? destination { get => _destination; set => SetProperty(ref _destination, value); }
        private long? _speed_bps;
        public long? speed_bps { get => _speed_bps; set { if (SetProperty(ref _speed_bps, value)) NotifyProgressProperties(); } }
        private long? _eta_secs;
        public long? eta_secs { get => _eta_secs; set { if (SetProperty(ref _eta_secs, value)) NotifyProgressProperties(); } }
        private bool _is_directory;
        public bool is_directory { get => _is_directory; set { if (SetProperty(ref _is_directory, value)) OnPropertyChanged(nameof(IsDirectory)); } }
        private int _item_count = 1;
        public int item_count { get => _item_count; set { if (SetProperty(ref _item_count, value)) OnPropertyChanged(nameof(ItemCount)); } }

        public string FileName => file_name;
        public bool IsDirectory => is_directory;
        public int ItemCount => item_count;
        public double PercentFloat => bytes_total > 0 ? ((double)bytes_received / bytes_total * 100) : 100.0;
        // JsonIgnore: "Percent"/"percent" collide under case-insensitive
        // matching, which was silently throwing on every ActiveTransfers
        // deserialization (see UpdateStateFromDaemon) - `percent` is the
        // real wire field, this is just a display-casing alias for it.
        [JsonIgnore]
        public int Percent => percent;
        public string PercentText => $"{PercentFloat:0.0}%";
        public string SizeText => bytes_total > 0 ? $"{DeskdropFormatting.FormatBytes(bytes_received)} / {DeskdropFormatting.FormatBytes(bytes_total)}" : DeskdropFormatting.FormatBytes(bytes_received);
        public string SpeedText => speed_bps.HasValue && speed_bps.Value > 0 ? $"{DeskdropFormatting.FormatBytes(speed_bps.Value)}/s" : "";
        public string EtaText => eta_secs.HasValue && eta_secs.Value > 0 ? $"{eta_secs.Value}s remaining" : "";
        public string StatusText
        {
            get
            {
                var from = string.IsNullOrWhiteSpace(from_device) ? "peer" : from_device;
                return status switch
                {
                    "incoming" => $"Waiting for approval from {from}",
                    "transferring" or "in_progress" => string.Join(" - ", new[] { SizeText, SpeedText, EtaText }.Where(s => !string.IsNullOrWhiteSpace(s))),
                    "paused" => $"Paused - {SizeText}",
                    "verifying" => "Verifying transfer integrity...",
                    "complete" or "completed" => "Complete",
                    "failed" => "Failed",
                    "cancelled" => "Cancelled",
                    _ => status
                };
            }
        }
        public string ProgressColor => status is "complete" or "completed" ? "#2AA971" : (status == "failed" ? "#D6483B" : "#3A66D8");

        // ---- Transfer-manager presentation ---------------------------
        //
        // The old row showed Accept, Reject *and* Cancel simultaneously for
        // every transfer regardless of state, which meant two of the three
        // were always wrong. These predicates give each state exactly the
        // actions that apply to it.

        public bool IsIncoming => status == "incoming";
        public bool IsInFlight => status is "transferring" or "in_progress" or "paused" or "verifying";
        public bool IsComplete => status is "complete" or "completed";
        public bool IsFailed => status is "failed" or "cancelled";
        public bool IsVerifying => status == "verifying";

        public bool ShowAcceptReject => IsIncoming;
        public bool ShowCancel => IsInFlight;
        public bool ShowProgress => IsIncoming || IsInFlight;
        public bool ShowOpenFolder => IsComplete;

        // Short state word for the row's status chip - the long StatusText
        // carries the detail underneath it.
        public string StateLabel => status switch
        {
            "incoming" => "Waiting for approval",
            "transferring" or "in_progress" => "Transferring",
            "paused" => "Paused",
            "verifying" => "Verifying",
            "complete" or "completed" => "Completed",
            "failed" => "Failed",
            "cancelled" => "Cancelled",
            _ => string.IsNullOrWhiteSpace(status) ? "Queued" : status,
        };

        public string StateColor => status switch
        {
            "complete" or "completed" => "#2AA971",
            "failed" or "cancelled" => "#D6483B",
            "incoming" => "#C9861E",
            "paused" => "#8A8A90",
            _ => "#3A66D8",
        };

        // "42.8 MB/s . 2 sec remaining" - the two numbers people actually
        // watch, joined only when both exist so there's never a dangling dot.
        public string RateText
        {
            get
            {
                var parts = new[] { SpeedText, EtaText }.Where(s => !string.IsNullOrWhiteSpace(s));
                return string.Join("  ·  ", parts);
            }
        }

        public string PeerLabel => string.IsNullOrWhiteSpace(from_device) ? "Unknown device" : from_device;

        public string PrimaryIcon => status switch
        {
            "incoming" => "Check",
            "transferring" or "in_progress" => "Pause",
            "paused" => "Play",
            "complete" or "completed" => "FolderOpen",
            "failed" => "RotateCcw",
            _ => "ShieldCheck"
        };
        public string PrimaryBackground => status == "incoming" ? "#34C759" : (status == "paused" ? "#0055CC" : "#0D000000");
        public string PrimaryForeground => status == "incoming" || status == "paused" ? "White" : "#D9000000";
        public bool PrimaryVisible => true;

        public bool SecondaryVisible => status == "incoming" || status == "in_progress" || status == "transferring" || status == "paused" || status == "verifying";
        public string SecondaryIcon => status == "incoming" ? "X" : "X";
        public string SecondaryBackground => "#1AFF3B30";
        public string SecondaryForeground => "#FF3B30";

        private void NotifyProgressProperties()
        {
            OnPropertyChanged(nameof(PercentFloat));
            OnPropertyChanged(nameof(PercentText));
            OnPropertyChanged(nameof(SizeText));
            OnPropertyChanged(nameof(SpeedText));
            OnPropertyChanged(nameof(EtaText));
            OnPropertyChanged(nameof(StatusText));
            OnPropertyChanged(nameof(ProgressColor));
            OnPropertyChanged(nameof(PrimaryVisible));
            OnPropertyChanged(nameof(PrimaryIcon));
            OnPropertyChanged(nameof(PrimaryBackground));
            OnPropertyChanged(nameof(PrimaryForeground));
            OnPropertyChanged(nameof(SecondaryVisible));
            OnPropertyChanged(nameof(IsIncoming));
            OnPropertyChanged(nameof(IsInFlight));
            OnPropertyChanged(nameof(IsComplete));
            OnPropertyChanged(nameof(IsFailed));
            OnPropertyChanged(nameof(IsVerifying));
            OnPropertyChanged(nameof(ShowAcceptReject));
            OnPropertyChanged(nameof(ShowCancel));
            OnPropertyChanged(nameof(ShowProgress));
            OnPropertyChanged(nameof(ShowOpenFolder));
            OnPropertyChanged(nameof(StateLabel));
            OnPropertyChanged(nameof(StateColor));
            OnPropertyChanged(nameof(RateText));
            OnPropertyChanged(nameof(PeerLabel));

            try
            {
                var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(App.MainWindow);
                if (status == "transferring" || status == "in_progress")
                {
                    TaskbarProgress.SetState(hwnd, TaskbarProgress.TaskbarStates.Normal);
                    TaskbarProgress.SetValue(hwnd, bytes_received, bytes_total > 0 ? bytes_total : 1);
                }
                else if (status == "paused")
                {
                    TaskbarProgress.SetState(hwnd, TaskbarProgress.TaskbarStates.Paused);
                    TaskbarProgress.SetValue(hwnd, bytes_received, bytes_total > 0 ? bytes_total : 1);
                }
                else if (status == "failed")
                {
                    TaskbarProgress.SetState(hwnd, TaskbarProgress.TaskbarStates.Error);
                    TaskbarProgress.SetValue(hwnd, 100, 100);
                }
                else if (status == "complete" || status == "completed")
                {
                    TaskbarProgress.SetState(hwnd, TaskbarProgress.TaskbarStates.NoProgress);
                }
            }
            catch (Exception ex) { App.HandleError(ex); }
        }
    }

    public class SpeedTestState : BaseViewModel
    {
        private string _peer_id = "";
        public string peer_id { get => _peer_id; set => SetProperty(ref _peer_id, value); }
        private string? _test_id;
        public string? test_id { get => _test_id; set => SetProperty(ref _test_id, value); }
        private string _phase = "";
        public string phase { get => _phase; set => SetProperty(ref _phase, value); }
        private long _bytes_transferred;
        public long bytes_transferred { get => _bytes_transferred; set { if (SetProperty(ref _bytes_transferred, value)) OnPropertyChanged(nameof(SpeedText)); } }
        private int _duration_secs;
        public int duration_secs { get => _duration_secs; set { if (SetProperty(ref _duration_secs, value)) OnPropertyChanged(nameof(SpeedText)); } }
        public string SpeedText
        {
            get
            {
                if (duration_secs <= 0) return "Starting...";
                var mbps = (bytes_transferred / Math.Max(1.0, duration_secs)) * 8.0 / 1_000_000.0;
                return $"{mbps:0.0} Mbps";
            }
        }
    }
    public class RemoteFile : BaseViewModel
    {
        [JsonPropertyName("file_id")]
        public ulong file_id { get; set; }
        
        [JsonPropertyName("id")]
        public string id { get; set; } = "";
        
        [JsonPropertyName("name")]
        public string name { get; set; } = "";
        
        [JsonPropertyName("display_name")]
        public string display_name { get; set; } = "";
        
        [JsonPropertyName("is_dir")]
        public bool is_dir { get; set; }
        
        [JsonPropertyName("size_bytes")]
        public long size_bytes { get; set; }
        
        [JsonPropertyName("size")]
        public long size { get; set; }
        
        [JsonPropertyName("date_modified")]
        public ulong date_modified { get; set; }
        
        [JsonPropertyName("modified_ms")]
        public ulong modified_ms { get; set; }

        [JsonPropertyName("mime_type")]
        public string mime_type { get; set; } = "";

        [JsonPropertyName("category")]
        public string category { get; set; } = "";

        [JsonPropertyName("source")]
        public string source { get; set; } = "";

        [JsonPropertyName("content_uri")]
        public string content_uri { get; set; } = "";
        
        private bool _isSelected;
        public bool IsSelected { get => _isSelected; set => SetProperty(ref _isSelected, value); }
        
        public long EffectiveSize => size_bytes > 0 ? size_bytes : size;
        public ulong EffectiveDate => date_modified > 0 ? (date_modified > 100000000000 ? date_modified : date_modified * 1000) : modified_ms;
        
        public string FormattedSize => is_dir ? "--" : DeskdropFormatting.FormatBytes(EffectiveSize);
        public string FormattedDate => EffectiveDate == 0 ? "--" : DateTimeOffset.FromUnixTimeMilliseconds((long)EffectiveDate).ToLocalTime().ToString("MMM dd, yyyy HH:mm");
        public string size_text => FormattedSize;
        public string modified_text => FormattedDate;
        public string IconKind => is_dir ? "Folder" : "File";
        public string IconColor => is_dir ? "#0055CC" : "#555555";

        // Thumbnail preview support (mirrors the macOS RemoteExplorerView:
        // only image/video files are previewable, fetched on demand from the
        // connected Android device over the existing peer link).
        public bool IsPreviewable => !is_dir && (category?.StartsWith("image", StringComparison.OrdinalIgnoreCase) == true
                                                  || category?.StartsWith("video", StringComparison.OrdinalIgnoreCase) == true);

        private Microsoft.UI.Xaml.Media.Imaging.BitmapImage? _thumbnail;
        public Microsoft.UI.Xaml.Media.Imaging.BitmapImage? Thumbnail
        {
            get => _thumbnail;
            set { if (SetProperty(ref _thumbnail, value)) OnPropertyChanged(nameof(HasThumbnail)); }
        }
        public bool HasThumbnail => _thumbnail != null;

        // Not bindable - just prevents re-requesting a thumbnail every time
        // this row's container is recycled/scrolled back into view.
        public bool ThumbnailRequested { get; set; }
    }

    public class RemoteFileListResponse
    {
        [JsonPropertyName("path")]
        public string path { get; set; } = "";
        
        [JsonPropertyName("files")]
        public List<RemoteFile> files { get; set; } = new();
        
        [JsonPropertyName("total_matching")]
        public uint total_matching { get; set; }
        
        [JsonPropertyName("error")]
        public string? error { get; set; }
    }


    public class PeerBatteryState
    {
        public string device_id { get; set; } = "";
        public string device_name { get; set; } = "";
        public int level { get; set; }
        public bool charging { get; set; }
    }

    public class PeerStorageState
    {
        public string device_id { get; set; } = "";
        public string device_name { get; set; } = "";
        public long images_bytes { get; set; }
        public long videos_bytes { get; set; }
        public long apps_bytes { get; set; }
        public long free_bytes { get; set; }
        public long total_bytes { get; set; }
    }

    public class ActiveCallState
    {
        public string device_id { get; set; } = "";
        public string device_name { get; set; } = "";
        public string state { get; set; } = "";
        public string number { get; set; } = "";
        public string contact_name { get; set; } = "";
    }

    public class DeskdropStore : BaseViewModel
    {
        public static DeskdropStore Shared { get; } = new DeskdropStore();
        private static readonly JsonSerializerOptions JsonOptions = new()
        {
            PropertyNameCaseInsensitive = true
        };

        private DeskdropStore()
        {
            Peers = new ObservableCollection<PeerViewModel>();
            History = new ObservableCollection<HistoryItem>();
            ActiveTransfers = new ObservableCollection<FileTransferState>();
            ActiveSpeedTests = new ObservableCollection<SpeedTestState>();
            ActivityFeed = new ObservableCollection<ActivityEntry>();
            PendingClipboards = new ObservableCollection<PendingClipboard>();
            Peers.CollectionChanged += (_, _) => NotifyPeerMetrics();
            ActiveTransfers.CollectionChanged += (_, _) => NotifyTransferMetrics();
            ActiveSpeedTests.CollectionChanged += (_, _) => NotifyTransferMetrics();
            ActivityFeed.CollectionChanged += (_, _) =>
            {
                OnPropertyChanged(nameof(ActivityCount));
                SyncRecentActivity();
            };
            PendingClipboards.CollectionChanged += (_, _) => NotifyPendingClipboardMetrics();
            
            StartPolling();
        }

        private System.Threading.Timer? _pollTimer;

        private void StartPolling()
        {
            UpdateStateFromDaemon();

            // The native engine starts asynchronously (see App.xaml.cs
            // OnLaunched Task.Run), so its IPC pipe often isn't up yet for
            // this first check - a one-shot poll can permanently freeze the
            // header on "Engine stopped" even once the engine is healthy.
            // Keep re-checking so the status self-corrects.
            _pollTimer = new System.Threading.Timer(
                _ => UpdateStateFromDaemon(),
                null,
                TimeSpan.FromSeconds(3),
                TimeSpan.FromSeconds(5));
        }



        private ObservableCollection<PeerViewModel> _peers = null!;
        public ObservableCollection<PeerViewModel> Peers
        {
            get => _peers;
            set { _peers = value; OnPropertyChanged(); }
        }

        private ObservableCollection<PeerViewModel> _connectedPeers = new ObservableCollection<PeerViewModel>();
        public ObservableCollection<PeerViewModel> ConnectedPeers
        {
            get => _connectedPeers;
            set { _connectedPeers = value; OnPropertyChanged(); }
        }

        // The Devices screen separates devices you've paired with from ones
        // merely visible on the network - they need different primary actions
        // and different visual weight, and lumping them into one list is what
        // made the old "Remembered Devices" list ambiguous.
        //
        // Both are kept as stable collection *instances* and synced in place,
        // so bound ListViews don't tear down and rebuild every card on the
        // 5-second poll.
        public ObservableCollection<PeerViewModel> KnownDevices { get; } = new();
        public ObservableCollection<PeerViewModel> NearbyDevices { get; } = new();
        public ObservableCollection<PeerViewModel> PairingRequests { get; } = new();

        // Newest few activity entries, for the Devices screen's summary
        // section. The full log lives on the Activity page.
        public ObservableCollection<ActivityEntry> RecentActivity { get; } = new();
        private const int RecentActivityLimit = 5;

        private PeerViewModel? _selectedPeer;
        public PeerViewModel? SelectedPeer
        {
            get => _selectedPeer;
            set { if (SetProperty(ref _selectedPeer, value)) _selectedPeer?.NotifyAll(); }
        }

        private ObservableCollection<HistoryItem> _history = null!;
        public ObservableCollection<HistoryItem> History
        {
            get => _history;
            set { _history = value; OnPropertyChanged(); }
        }

        private ObservableCollection<FileTransferState> _activeTransfers = null!;
        public ObservableCollection<FileTransferState> ActiveTransfers
        {
            get => _activeTransfers;
            set { _activeTransfers = value; OnPropertyChanged(); }
        }

        private ObservableCollection<SpeedTestState> _activeSpeedTests = null!;
        public ObservableCollection<SpeedTestState> ActiveSpeedTests
        {
            get => _activeSpeedTests;
            set { _activeSpeedTests = value; OnPropertyChanged(); }
        }

        private ObservableCollection<ActivityEntry> _activityFeed = null!;
        public ObservableCollection<ActivityEntry> ActivityFeed
        {
            get => _activityFeed;
            set { _activityFeed = value; OnPropertyChanged(); }
        }

        private ObservableCollection<PendingClipboard> _pendingClipboards = null!;
        public ObservableCollection<PendingClipboard> PendingClipboards
        {
            get => _pendingClipboards;
            set { _pendingClipboards = value; OnPropertyChanged(); }
        }

        private ActiveCallState? _activeCall;
        public ActiveCallState? ActiveCall
        {
            get => _activeCall;
            set { _activeCall = value; OnPropertyChanged(); }
        }

        private bool _isDaemonRunning;
        public bool IsDaemonRunning
        {
            get => _isDaemonRunning;
            set
            {
                if (SetProperty(ref _isDaemonRunning, value))
                {
                    OnPropertyChanged(nameof(HeaderStatusText));
                    OnPropertyChanged(nameof(HeaderStatusBrush));
                    OnPropertyChanged(nameof(DaemonStatusText));
                    OnPropertyChanged(nameof(IsSearching));
                    OnPropertyChanged(nameof(EcosystemSummaryText));
                }
            }
        }

        private string _statusLine = "Starting...";
        public string StatusLine
        {
            get => _statusLine;
            set { _statusLine = value; OnPropertyChanged(); }
        }

        public int PeerCount => Peers?.Count ?? 0;
        public bool HasPeers => Peers != null && Peers.Count > 0;
        public bool HasNoPeers => !HasPeers;
        public int ConnectedCount => Peers?.Count(p => p.IsConnected) ?? 0;
        public int TrustedCount => Peers?.Count(p => p.is_trusted) ?? 0;
        public int AttentionCount => Peers?.Count(p => !p.is_trusted || p.pairingRequested || p.outgoingPairingWaiting) ?? 0;
        public int ActivityCount => ActivityFeed?.Count ?? 0;
        public int PendingClipboardCount => PendingClipboards?.Count ?? 0;
        public bool HasPendingClipboards => PendingClipboardCount > 0;
        // Exposed as a notified property rather than letting views bind to
        // ActiveTransfers.Count: x:Bind on a collection's Count reads it once
        // and never hears CollectionChanged, so those counters silently went
        // stale as transfers came and went.
        public int ActiveTransferCount => ActiveTransfers?.Count ?? 0;
        public bool HasActiveTransfers => (ActiveTransfers?.Count ?? 0) > 0;
        public bool HasActiveSpeedTests => (ActiveSpeedTests?.Count ?? 0) > 0;
        private bool _otpShieldEnabled = true;
        public bool OtpShieldEnabled { get => _otpShieldEnabled; set => SetProperty(ref _otpShieldEnabled, value); }
        private bool _syncEnabled = true;
        public bool SyncEnabled
        {
            get => _syncEnabled;
            set
            {
                if (SetProperty(ref _syncEnabled, value))
                {
                    DaemonClient.SetSyncEnabled(value);
                    OnPropertyChanged(nameof(ClipboardSummaryText));
                }
            }
        }
        private bool _requireTofuConfirmation = true;
        public bool RequireTofuConfirmation { get => _requireTofuConfirmation; set { if (SetProperty(ref _requireTofuConfirmation, value)) DaemonClient.PatchSettings(new { require_tofu_confirmation = value }); } }
        public string DaemonStatusText => IsDaemonRunning ? "Running" : "Stopped";
        public string HeaderStatusText
        {
            get
            {
                if (!IsDaemonRunning) return "Engine stopped";
                if (ConnectedCount > 0) return $"{ConnectedCount} connected";
                if (AttentionCount > 0) return "Ready to pair";
                return "Looking for devices";
            }
        }

        public string HeaderStatusBrush
        {
            get
            {
                if (!IsDaemonRunning) return "#D6483B";
                if (ConnectedCount > 0) return "#2AA971";
                if (AttentionCount > 0) return "#C9861E";
                return "#8A8A90";
            }
        }

        // ---- Derived state for the redesigned shell -------------------

        // Drives the header indicator's pulse. Animation here means "work in
        // progress", so it runs only while we're actually looking for
        // something - a connected, settled state gets a steady dot.
        public bool IsSearching => IsDaemonRunning && ConnectedCount == 0;

        // Distinguishes "we haven't asked the engine yet" from "we asked and
        // there is nothing". Without it, the Devices page flashes its
        // "No devices paired yet" empty state for the first second of every
        // launch, which reads as data loss.
        private bool _hasLoadedOnce;
        public bool HasLoadedOnce
        {
            get => _hasLoadedOnce;
            set
            {
                if (SetProperty(ref _hasLoadedOnce, value))
                {
                    OnPropertyChanged(nameof(IsInitialLoad));
                    OnPropertyChanged(nameof(ShowDevicesEmptyState));
                }
            }
        }

        public bool IsInitialLoad => !HasLoadedOnce;

        // The empty state is only honest once we've actually heard back.
        public bool ShowDevicesEmptyState => HasLoadedOnce && KnownDevices.Count == 0;

        public int KnownDeviceCount => KnownDevices.Count;
        public int NearbyDeviceCount => NearbyDevices.Count;
        public bool HasKnownDevices => KnownDevices.Count > 0;
        public bool HasNoKnownDevices => KnownDevices.Count == 0;
        public bool HasNearbyDevices => NearbyDevices.Count > 0;
        public bool HasPairingRequests => PairingRequests.Count > 0;
        public bool HasRecentActivity => RecentActivity.Count > 0;
        public bool HasNoRecentActivity => RecentActivity.Count == 0;

        // The one line that has to answer "is my ecosystem healthy?" in
        // under a second. Reachability first, then trust, then encryption -
        // in that order, because that's the order the user cares about.
        public string EcosystemSummaryText
        {
            get
            {
                if (!IsDaemonRunning) return "Local service stopped  ·  Deskdrop can't reach the network";
                if (ConnectedCount == 1) return "1 device connected  ·  Encrypted local network";
                if (ConnectedCount > 1) return $"{ConnectedCount} devices connected  ·  Encrypted local network";
                if (HasKnownDevices) return "No devices connected  ·  Listening on the local network";
                return "No devices paired yet  ·  Listening on the local network";
            }
        }

        // Compact one-liner for the in-progress banner: how many, how fast.
        public string ActiveTransferSummaryText
        {
            get
            {
                var count = ActiveTransfers?.Count ?? 0;
                if (count == 0) return "";

                var noun = count == 1 ? "1 transfer" : $"{count} transfers";
                var fastest = ActiveTransfers!
                    .Where(t => t.speed_bps.HasValue && t.speed_bps.Value > 0)
                    .Select(t => t.speed_bps!.Value)
                    .DefaultIfEmpty(0)
                    .Max();

                return fastest > 0
                    ? $"{noun} in progress  ·  {DeskdropFormatting.FormatBytes(fastest)}/s"
                    : $"{noun} in progress";
            }
        }

        // Header line for the clipboard bridge: who it's sharing with, and
        // whether automatic sync is actually on - the two things that explain
        // why an entry did or didn't appear.
        public string ClipboardSummaryText
        {
            get
            {
                var scope = ConnectedCount switch
                {
                    0 => "No connected devices",
                    1 => "Sharing with 1 device",
                    _ => $"Sharing with {ConnectedCount} devices",
                };
                return $"{scope}  ·  Auto-sync {(SyncEnabled ? "on" : "off")}";
            }
        }

        // Header line for the transfer manager: what's happening now, and
        // where finished files land - the two questions this page gets asked.
        public string TransfersSummaryText
        {
            get
            {
                var active = ActiveTransfers?.Count ?? 0;
                if (active > 0) return ActiveTransferSummaryText;
                if (HasActiveSpeedTests) return "Benchmark running";
                return "No active transfers  ·  Received files are saved to your Downloads folder";
            }
        }

        private int _isRefreshInFlight = 0;

        public void ConnectAndPair(string deviceId)
        {
            DaemonClient.SendPairingRequest(deviceId);
        }

        public void RespondToPairing(string deviceId, bool accepted)
        {
            DaemonClient.RespondToPairing(deviceId, accepted);
        }

        public void DisconnectPeer(string deviceId)
        {
            DaemonClient.DisconnectPeer(deviceId);
        }

        public void ForgetPeer(string deviceId)
        {
            DaemonClient.ForgetDevice(deviceId);
        }

        public void AcceptTransfer(string transferId)
        {
            DaemonClient.AcceptFileTransfer(transferId);
        }

        public void RejectTransfer(string transferId)
        {
            DaemonClient.RejectFileTransfer(transferId, "user_declined");
        }

        public void ApplyClipboardItem(string contentHash)
        {
            DaemonClient.ApplyClipboard(contentHash);
        }

        public void SendPushText(string text, string toDeviceId)
        {
            DaemonClient.PushTextTo(text, toDeviceId);
        }

        public void UpdateStateFromDaemon()
        {
            if (System.Threading.Interlocked.CompareExchange(ref _isRefreshInFlight, 1, 0) != 0) return;

            System.Threading.Tasks.Task.Run(() =>
            {
                try
                {
                    bool isRunning = DaemonClient.IsDaemonRunning();
                    
                    App.MainWindow?.DispatcherQueue?.TryEnqueue(() =>
                    {
                        try { IsDaemonRunning = isRunning; } catch (Exception ex) { App.HandleError(ex); }
                    });

                    if (isRunning)
                    {
                        var state = DaemonClient.Status();
                        if (state != null && state.RootElement.TryGetProperty("data", out var dataElem))
                        {
                            ParseDaemonState(dataElem);
                        }

                        var activity = DaemonClient.ActivityRecent(80);
                        if (activity != null && activity.RootElement.TryGetProperty("data", out var actDataElem))
                        {
                            ParseActivityFeed(actDataElem);
                        }

                        var pending = DaemonClient.PendingRemoteClipboards();
                        if (pending != null && pending.RootElement.TryGetProperty("data", out var pendDataElem))
                        {
                            ParsePendingClipboards(pendDataElem);
                        }

                        var settings = DaemonClient.GetSettings();
                        if (settings != null && settings.RootElement.TryGetProperty("data", out var settingsDataElem))
                        {
                            if (settingsDataElem.TryGetProperty("require_tofu_confirmation", out var tofuElem))
                            {
                                bool tofu = tofuElem.GetBoolean();
                                App.MainWindow?.DispatcherQueue?.TryEnqueue(() =>
                                {
                                    // Set the backing field directly (not the public setter) so
                                    // loading the daemon's current value doesn't turn around and
                                    // PatchSettings it straight back.
                                    if (_requireTofuConfirmation != tofu)
                                    {
                                        _requireTofuConfirmation = tofu;
                                        OnPropertyChanged(nameof(RequireTofuConfirmation));
                                    }
                                });
                            }
                        }
                    }
                }
                catch (Exception ex)
                {
                    // Handle failure gracefully
                    App.MainWindow?.DispatcherQueue?.TryEnqueue(() =>
                    {
                        StatusLine = $"Error connecting to daemon: {ex.Message}";
                    });
                }
                finally
                {
                    System.Threading.Interlocked.Exchange(ref _isRefreshInFlight, 0);

                    // First poll is done - the UI can stop showing skeletons
                    // and start trusting "no devices" to mean no devices.
                    App.MainWindow?.DispatcherQueue?.TryEnqueue(() => HasLoadedOnce = true);
                }
            });
        }

                private void ParseActivityFeed(JsonElement dataElem)
        {
            var entries = DeserializeList<ActivityEntry>(dataElem, "entries");
            if (entries != null)
            {
                App.MainWindow?.DispatcherQueue?.TryEnqueue(() =>
                {
                    try
                    {
                        ActivityFeed = new ObservableCollection<ActivityEntry>(entries.OrderByDescending(e => e.timestamp_ms));
                        OnPropertyChanged(nameof(ActivityCount));
                    }
                    catch (Exception ex) { App.HandleError(ex); }
                });
            }
        }

                private void ParsePendingClipboards(JsonElement dataElem)
        {
            var clips = DeserializeList<PendingClipboard>(dataElem, "clipboards");
            if (clips != null)
            {
                App.MainWindow?.DispatcherQueue?.TryEnqueue(() =>
                {
                    try
                    {
                        PendingClipboards = new ObservableCollection<PendingClipboard>(clips.OrderByDescending(c => c.timestamp_ms));
                        NotifyPendingClipboardMetrics();
                    }
                    catch (Exception ex) { App.HandleError(ex); }
                });
            }
        }

        private void ParseDaemonState(JsonElement dataElem)
        {
            System.Collections.Generic.List<PeerViewModel>? newPeers = null;
            System.Collections.Generic.List<PeerBatteryState>? batteries = null;
            System.Collections.Generic.List<PeerStorageState>? storages = null;

            if (dataElem.TryGetProperty("peers", out var peersElem))
                newPeers = JsonSerializer.Deserialize<System.Collections.Generic.List<PeerViewModel>>(peersElem.GetRawText(), JsonOptions);
            
            if (dataElem.TryGetProperty("peer_batteries", out var batElem))
                batteries = JsonSerializer.Deserialize<System.Collections.Generic.List<PeerBatteryState>>(batElem.GetRawText(), JsonOptions);

            if (dataElem.TryGetProperty("peer_storages", out var storElem))
                storages = JsonSerializer.Deserialize<System.Collections.Generic.List<PeerStorageState>>(storElem.GetRawText(), JsonOptions);

            if (newPeers != null)
            {
                foreach (var peer in newPeers)
                {
                    if (batteries != null)
                    {
                        var bat = batteries.Find(b => b.device_id == peer.device_id);
                        if (bat != null)
                        {
                            peer.BatteryLevel = bat.level;
                            peer.BatteryCharging = bat.charging;
                        }
                    }
                    if (storages != null)
                    {
                        var st = storages.Find(s => s.device_id == peer.device_id);
                        if (st != null)
                        {
                            peer.StorageTotal = st.total_bytes;
                            peer.StorageFree = st.free_bytes;
                            peer.StorageImages = st.images_bytes;
                            peer.StorageVideos = st.videos_bytes;
                            peer.StorageApps = st.apps_bytes;
                        }
                    }
                }
            }

            App.MainWindow?.DispatcherQueue?.TryEnqueue(() =>
            {
                try
                {
                if (newPeers != null)
                {
                    // Update existing PeerViewModel instances in place (matched by
                    // device_id) instead of replacing the whole collection every
                    // poll - preserving object identity keeps the bound ListView
                    // from tearing down and rebuilding every device card (which
                    // read as constant "flicker"/refresh and reset hover state)
                    // on every 5s poll. Mirrors the ActiveTransfers merge below.
                    var stalePeers = Peers.ToList();
                    foreach (var incoming in newPeers)
                    {
                        var match = Peers.FirstOrDefault(p => p.device_id == incoming.device_id);
                        if (match != null)
                        {
                            match.friendly_name = incoming.friendly_name;
                            match.platform = incoming.platform;
                            match.status = incoming.status;
                            match.is_trusted = incoming.is_trusted;
                            match.remembered = incoming.remembered;
                            match.sync_enabled = incoming.sync_enabled;
                            match.remote_sync_enabled = incoming.remote_sync_enabled;
                            match.auto_connect = incoming.auto_connect;
                            match.explicit_disconnect = incoming.explicit_disconnect;
                            match.last_seen = incoming.last_seen;
                            match.last_error = incoming.last_error;
                            match.ips = incoming.ips;
                            match.fingerprint_display = incoming.fingerprint_display;
                            match.first_seen = incoming.first_seen;
                            match.pairingPin = incoming.pairingPin;
                            match.pairingRequested = incoming.pairingRequested;
                            match.outgoingPairingWaiting = incoming.outgoingPairingWaiting;
                            match.BatteryLevel = incoming.BatteryLevel;
                            match.BatteryCharging = incoming.BatteryCharging;
                            match.StorageTotal = incoming.StorageTotal;
                            match.StorageFree = incoming.StorageFree;
                            match.StorageImages = incoming.StorageImages;
                            match.StorageVideos = incoming.StorageVideos;
                            match.StorageApps = incoming.StorageApps;
                            stalePeers.Remove(match);
                        }
                        else
                        {
                            Peers.Add(incoming);
                        }
                    }
                    foreach (var stale in stalePeers) Peers.Remove(stale);

                    var connected = Peers.Where(p => p.is_trusted && p.status == "connected").ToList();
                    ConnectedPeers = new ObservableCollection<PeerViewModel>(connected);

                    StatusLine = Peers.Count == 0 ? "Running - no devices connected" : $"Connected to {ConnectedCount} device{(ConnectedCount == 1 ? "" : "s")}";
                    NotifyPeerMetrics();
                }

                if (dataElem.TryGetProperty("active_transfers", out var transfersElem))
                {
                    var transfers = JsonSerializer.Deserialize<System.Collections.Generic.List<FileTransferState>>(transfersElem.GetRawText(), JsonOptions);
                    if (transfers != null)
                    {
                        var existing = ActiveTransfers.ToList();
                        foreach (var tr in transfers)
                        {
                            var match = ActiveTransfers.FirstOrDefault(t => t.transfer_id == tr.transfer_id);
                            if (match != null)
                            {
                                match.status = tr.status;
                                match.from_device = tr.from_device;
                                match.file_name = tr.file_name;
                                match.bytes_received = tr.bytes_received;
                                match.bytes_total = tr.bytes_total;
                                match.percent = tr.percent;
                                match.destination = tr.destination;
                                match.speed_bps = tr.speed_bps;
                                match.eta_secs = tr.eta_secs;
                                existing.Remove(match);
                            }
                            else
                            {
                                ActiveTransfers.Add(tr);
                            }
                        }
                        foreach(var rem in existing) ActiveTransfers.Remove(rem);
                        NotifyTransferMetrics();
                    }
                }
                else
                {
                    ActiveTransfers.Clear();
                    NotifyTransferMetrics();
                }

                if (dataElem.TryGetProperty("active_speed_tests", out var speedElem))
                {
                    var speedTests = JsonSerializer.Deserialize<System.Collections.Generic.List<SpeedTestState>>(speedElem.GetRawText(), JsonOptions);
                    if (speedTests != null)
                    {
                        var existing = ActiveSpeedTests.ToList();
                        foreach (var st in speedTests)
                        {
                            var key = string.IsNullOrWhiteSpace(st.peer_id) ? st.test_id : st.peer_id;
                            var match = ActiveSpeedTests.FirstOrDefault(t => (string.IsNullOrWhiteSpace(t.peer_id) ? t.test_id : t.peer_id) == key);
                            if (match != null)
                            {
                                match.peer_id = st.peer_id;
                                match.test_id = st.test_id;
                                match.phase = st.phase;
                                match.bytes_transferred = st.bytes_transferred;
                                match.duration_secs = st.duration_secs;
                                existing.Remove(match);
                            }
                            else
                            {
                                ActiveSpeedTests.Add(st);
                            }
                        }
                        foreach (var rem in existing) ActiveSpeedTests.Remove(rem);
                        NotifyTransferMetrics();
                    }
                }
                else
                {
                    ActiveSpeedTests.Clear();
                    NotifyTransferMetrics();
                }

                if (dataElem.TryGetProperty("active_call", out var callElem) && callElem.ValueKind != JsonValueKind.Null)
                {
                    ActiveCall = JsonSerializer.Deserialize<ActiveCallState>(callElem.GetRawText());
                }
                else
                {
                    ActiveCall = null;
                }
                }
                catch (Exception ex) { App.HandleError(ex); }
            });
        }

        public void TriggerHistoryUpdate()
        {
            OnPropertyChanged(nameof(History));
        }

        private static System.Collections.Generic.List<T>? DeserializeList<T>(JsonElement element, string wrapperName)
        {
            try
            {
                if (element.ValueKind == JsonValueKind.Array)
                {
                    return JsonSerializer.Deserialize<System.Collections.Generic.List<T>>(element.GetRawText(), JsonOptions);
                }
                if (element.ValueKind == JsonValueKind.Object && element.TryGetProperty(wrapperName, out var wrapped) && wrapped.ValueKind == JsonValueKind.Array)
                {
                    return JsonSerializer.Deserialize<System.Collections.Generic.List<T>>(wrapped.GetRawText(), JsonOptions);
                }
            }
            catch (Exception ex) { App.HandleError(ex); }
            return null;
        }

        private static void CopyActivityEntry(ActivityEntry source, ActivityEntry target)
        {
            target.kind = source.kind;
            target.summary = source.summary;
            target.timestamp_ms = source.timestamp_ms;
            target.device_id = source.device_id;
            target.device_name = source.device_name;
            target.content_hash = source.content_hash;
            target.text_preview = source.text_preview;
            target.file_name = source.file_name;
            target.file_bytes = source.file_bytes;
            target.transfer_id = source.transfer_id;
            target.dest_path = source.dest_path;
            target.applied_locally = source.applied_locally;
            target.relay_path = source.relay_path;
        }

        private void NotifyPeerMetrics()
        {
            if (SelectedPeer == null && Peers.Count > 0)
            {
                SelectedPeer = Peers.FirstOrDefault(p => p.IsConnected) ?? Peers.FirstOrDefault();
            }
            OnPropertyChanged(nameof(SelectedPeer));
            SelectedPeer?.NotifyAll();
            OnPropertyChanged(nameof(PeerCount));
            OnPropertyChanged(nameof(HasPeers));
            OnPropertyChanged(nameof(HasNoPeers));
            OnPropertyChanged(nameof(ConnectedCount));
            OnPropertyChanged(nameof(TrustedCount));
            OnPropertyChanged(nameof(AttentionCount));
            OnPropertyChanged(nameof(HeaderStatusText));
            OnPropertyChanged(nameof(HeaderStatusBrush));

            SyncPeerProjection(KnownDevices, Peers.Where(p => p.IsKnown));
            SyncPeerProjection(NearbyDevices, Peers.Where(p => p.IsNearby));
            SyncPeerProjection(PairingRequests, Peers.Where(p => p.pairingRequested));

            OnPropertyChanged(nameof(KnownDeviceCount));
            OnPropertyChanged(nameof(NearbyDeviceCount));
            OnPropertyChanged(nameof(HasKnownDevices));
            OnPropertyChanged(nameof(HasNoKnownDevices));
            OnPropertyChanged(nameof(HasNearbyDevices));
            OnPropertyChanged(nameof(HasPairingRequests));
            OnPropertyChanged(nameof(EcosystemSummaryText));
            OnPropertyChanged(nameof(IsSearching));
            OnPropertyChanged(nameof(ClipboardSummaryText));
            OnPropertyChanged(nameof(ShowDevicesEmptyState));
        }

        // Reconciles a filtered projection against the master Peers list
        // without replacing the collection instance. Removing stale entries
        // first, then inserting missing ones at their target index, keeps
        // object identity intact - which is what stops every device card from
        // being rebuilt (losing hover and focus) on each poll.
        private static void SyncPeerProjection(ObservableCollection<PeerViewModel> target, IEnumerable<PeerViewModel> source)
        {
            var desired = source.ToList();

            for (var i = target.Count - 1; i >= 0; i--)
            {
                if (!desired.Contains(target[i])) target.RemoveAt(i);
            }

            for (var i = 0; i < desired.Count; i++)
            {
                var item = desired[i];
                var existing = target.IndexOf(item);
                if (existing < 0) target.Insert(i, item);
                else if (existing != i) target.Move(existing, i);
            }
        }

        // Mirrors in-flight transfers onto the devices they belong to, so a
        // device row can show it's busy without every row subscribing to the
        // whole transfer list. Matching is by display name because that's the
        // only peer identity the transfer payload carries.
        private void SyncTransferringPeers()
        {
            if (Peers == null) return;

            var busy = (ActiveTransfers ?? new ObservableCollection<FileTransferState>())
                .Where(t => t.IsInFlight)
                .Select(t => t.from_device)
                .Where(n => !string.IsNullOrWhiteSpace(n))
                .ToHashSet(StringComparer.OrdinalIgnoreCase);

            foreach (var peer in Peers)
            {
                peer.IsTransferring = busy.Contains(peer.DisplayName) || busy.Contains(peer.friendly_name);
            }
        }

        // Keeps the Devices screen's activity preview to the newest handful.
        private void SyncRecentActivity()
        {
            if (ActivityFeed == null) return;

            var desired = ActivityFeed.Take(RecentActivityLimit).ToList();

            for (var i = RecentActivity.Count - 1; i >= 0; i--)
            {
                if (!desired.Contains(RecentActivity[i])) RecentActivity.RemoveAt(i);
            }

            for (var i = 0; i < desired.Count; i++)
            {
                var item = desired[i];
                var existing = RecentActivity.IndexOf(item);
                if (existing < 0) RecentActivity.Insert(i, item);
                else if (existing != i) RecentActivity.Move(existing, i);
            }

            OnPropertyChanged(nameof(HasRecentActivity));
            OnPropertyChanged(nameof(HasNoRecentActivity));
        }

        private bool _sleepImmunityActive;

        private void NotifyTransferMetrics()
        {
            OnPropertyChanged(nameof(HasActiveTransfers));
            OnPropertyChanged(nameof(ActiveTransferCount));
            OnPropertyChanged(nameof(HasActiveSpeedTests));
            OnPropertyChanged(nameof(ActiveTransferSummaryText));
            OnPropertyChanged(nameof(TransfersSummaryText));
            SyncTransferringPeers();

            // Prevent Modern Standby / display sleep for as long as a
            // transfer or speed test is in flight, mirroring macOS's
            // ProcessInfo.beginActivity and Android's wake lock behaviour.
            bool shouldStayAwake = HasActiveTransfers || HasActiveSpeedTests;
            if (shouldStayAwake != _sleepImmunityActive)
            {
                _sleepImmunityActive = shouldStayAwake;
                try
                {
                    NativeCore.SetThreadExecutionState(shouldStayAwake
                        ? (NativeCore.ES_CONTINUOUS | NativeCore.ES_SYSTEM_REQUIRED)
                        : NativeCore.ES_CONTINUOUS);
                }
                catch (Exception ex) { App.HandleError(ex); }
            }
        }

        private void NotifyPendingClipboardMetrics()
        {
            OnPropertyChanged(nameof(PendingClipboardCount));
            OnPropertyChanged(nameof(HasPendingClipboards));
        }

        public async System.Threading.Tasks.Task PickAndSendFiles(string targetDeviceId)
        {
            try
            {
                if (App.MainWindow == null) return;
                var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(App.MainWindow);
                if (hwnd == IntPtr.Zero) return;

                var picker = new Windows.Storage.Pickers.FileOpenPicker();
                picker.ViewMode = Windows.Storage.Pickers.PickerViewMode.List;
                picker.SuggestedStartLocation = Windows.Storage.Pickers.PickerLocationId.DocumentsLibrary;
                picker.FileTypeFilter.Add("*");

                WinRT.Interop.InitializeWithWindow.Initialize(picker, hwnd);

                var files = await picker.PickMultipleFilesAsync();
                if (files != null && files.Count > 0)
                {
                    foreach (var f in files)
                    {
                        _ = System.Threading.Tasks.Task.Run(() => DaemonClient.PushFile(targetDeviceId, f.Path));
                    }
                }
            }
            catch (Exception ex) { App.HandleError(ex); }
        }
    }

    public class RatioToStarConverter : Microsoft.UI.Xaml.Data.IValueConverter
    {
        public object Convert(object value, Type targetType, object parameter, string language)
        {
            if (value is double ratio)
            {
                return new GridLength(ratio, GridUnitType.Star);
            }
            return new GridLength(0);
        }

        public object ConvertBack(object value, Type targetType, object parameter, string language)
        {
            throw new NotImplementedException();
        }
    }

    internal static class DeskdropFormatting
    {
        internal static string FormatBytes(long bytes)
        {
            if (bytes < 1024) return $"{bytes} B";
            double kb = bytes / 1024.0;
            if (kb < 1024) return $"{kb:0.#} KB";
            double mb = kb / 1024.0;
            if (mb < 1024) return $"{mb:0.#} MB";
            return $"{mb / 1024.0:0.#} GB";
        }

        internal static string RelativeTimeFromUnixMs(ulong timestampMs)
        {
            try
            {
                var date = DateTimeOffset.FromUnixTimeMilliseconds((long)timestampMs);
                return RelativeTimeFrom(date);
            }
            catch { return "Just now"; }
        }

        internal static string RelativeTimeFromUnixSeconds(ulong timestampSeconds)
        {
            try
            {
                var date = DateTimeOffset.FromUnixTimeSeconds((long)timestampSeconds);
                return RelativeTimeFrom(date);
            }
            catch { return ""; }
        }

        private static string RelativeTimeFrom(DateTimeOffset date)
        {
            var delta = DateTimeOffset.Now - date;
            if (delta.TotalSeconds < 45) return "Just now";
            if (delta.TotalMinutes < 60) return $"{(int)delta.TotalMinutes}m ago";
            if (delta.TotalHours < 24) return $"{(int)delta.TotalHours}h ago";
            if (delta.TotalDays < 7) return $"{(int)delta.TotalDays}d ago";
            return date.LocalDateTime.ToString("MMM d");
        }
    }
}













namespace Deskdrop.WinUI { 
    public class HistoryItem : BaseViewModel { 
        public string id {get;set;} = Guid.NewGuid().ToString();
        public string display_text {get;set;} = "";
        public string path {get;set;} = "";
        public bool is_text {get;set;} = true;
        private bool _isPinned;
        public bool IsPinned
        {
            get => _isPinned;
            set
            {
                if (SetProperty(ref _isPinned, value))
                {
                    OnPropertyChanged(nameof(PinColor));
                    OnPropertyChanged(nameof(PinTooltip));
                }
            }
        }
        public string PinColor => IsPinned ? "#3A66D8" : "#8A8A90";

        // Windows glyph for the row. TypeIcon below is the emoji used by the
        // cross-platform surfaces; on Windows we want Segoe Fluent so the
        // icon set stays internally consistent. Built from a code point so
        // this file stays pure ASCII.
        public string Glyph => is_text ? char.ConvertFromUtf32(0xE8C8) : char.ConvertFromUtf32(0xE8A5);
        public bool HasPath => !string.IsNullOrWhiteSpace(path);
        public string PinTooltip => IsPinned ? "Unpin from the top" : "Pin to the top";

        public string TypeIcon { get; set; } = "📝";
        public string Summary { get; set; } = "";
        public string FullText { get; set; } = "";
        public string Source { get; set; } = "";
        public string RelativeTime { get; set; } = "Just now";
        public DateTime Time { get; set; } = DateTime.Now;
        // JsonIgnore: same "collides under case-insensitive matching" issue
        // as ActivityEntry.Source/FileTransferState.Percent above - latent
        // here since HistoryItem isn't JSON round-tripped today, but fixing
        // it before it becomes a live bug.
        [JsonIgnore]
        public string Id { get => id; set => id = value; }
    }
}











