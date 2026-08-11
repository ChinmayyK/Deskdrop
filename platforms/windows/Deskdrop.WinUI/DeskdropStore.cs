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
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
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

        // Compatibility for older in-process history bindings.
        public ulong timestamp { get => timestamp_ms; set => timestamp_ms = value; }
        public string source { get => device_name; set => device_name = value; }

        public string Title => !string.IsNullOrWhiteSpace(file_name)
            ? file_name!
            : (!string.IsNullOrWhiteSpace(text_preview) ? text_preview! : summary);
        public string Preview => !string.IsNullOrWhiteSpace(text_preview) ? text_preview! : summary;
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
        public string AccentColor => kind switch
        {
            "remote_clipboard_available" => "#0055CC",
            "clipboard_applied" => "#34C759",
            "clipboard_image" => "#5E5CE6",
            "file_transfer_complete" => "#34C759",
            "file_transfer_failed" => "#FF3B30",
            "peer_connected" => "#34C759",
            "peer_disconnected" => "#8E8E93",
            "sync_paused" => "#FF9500",
            "remote_notification" => "#5E5CE6",
            _ => "#0055CC"
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
        public string? platform { get => _platform; set { if (SetProperty(ref _platform, value)) OnPropertyChanged(nameof(DeviceIcon)); } }
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
        public string ConnectionColor => pairingRequested || outgoingPairingWaiting ? "#FF9500" : (status == "connected" ? "#34C759" : "#8E8E93");
        public string TrustText => is_trusted ? "Trusted" : "Pairing required";
        public string TrustColor => is_trusted ? "#34C759" : "#FF9500";
        public string DeviceIcon => (platform ?? friendly_name).ToLowerInvariant() switch
        {
            var p when p.Contains("windows") => "Monitor",
            var p when p.Contains("mac") => "Laptop",
            var p when p.Contains("linux") => "Server",
            _ => "Smartphone"
        };
        public string LastSeenText => last_seen.HasValue ? $"Seen {DeskdropFormatting.RelativeTimeFromUnixSeconds(last_seen.Value)}" : "";
        public bool HasError => !string.IsNullOrWhiteSpace(last_error);
        public bool IsConnected => status == "connected";
        
        public bool ShowVerifyButton => !is_trusted;
        public bool ShowDisconnectButton => status == "connected";
        public bool ShowConnectButton => status != "connected" && is_trusted;
        public bool ShowForgetButton => true;

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
        public string BatteryColor => BatteryCharging ? "#34C759" : (BatteryLevel <= 20 ? "#FF3B30" : "#8E8E93");

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
        }

        public void NotifyAll()
        {
            NotifyPeerStateProperties();
            NotifyStorageProperties();
            OnPropertyChanged(nameof(DisplayName));
            OnPropertyChanged(nameof(ShowBattery));
            OnPropertyChanged(nameof(BatteryIcon));
            OnPropertyChanged(nameof(BatteryColor));
            OnPropertyChanged(nameof(pairingPin));
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
        public string ProgressColor => status is "complete" or "completed" ? "#34C759" : (status == "failed" ? "#FF3B30" : "#0055CC");

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
        public string IconKind => is_dir ? "Folder" : "File";
        public string IconColor => is_dir ? "#0055CC" : "#555555";
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

        private DispatcherTimer? _pollTimer;

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
            ActivityFeed.CollectionChanged += (_, _) => OnPropertyChanged(nameof(ActivityCount));
            PendingClipboards.CollectionChanged += (_, _) => NotifyPendingClipboardMetrics();
            
            StartPolling();
        }

        private void StartPolling()
        {
            UpdateStateFromDaemon();
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
                }
            }
        }

        private string _statusLine = "Starting...";
        public string StatusLine
        {
            get => _statusLine;
            set { _statusLine = value; OnPropertyChanged(); }
        }

        public int ConnectedCount => Peers.Count(p => p.IsConnected);
        public int TrustedCount => Peers.Count(p => p.is_trusted);
        public int AttentionCount => Peers.Count(p => !p.is_trusted || p.pairingRequested || p.outgoingPairingWaiting);
        public int ActivityCount => ActivityFeed.Count;
        public int PendingClipboardCount => PendingClipboards.Count;
        public bool HasPendingClipboards => PendingClipboardCount > 0;
        public bool HasActiveTransfers => ActiveTransfers.Count > 0;
        public bool HasActiveSpeedTests => ActiveSpeedTests.Count > 0;
        private bool _otpShieldEnabled = true;
        public bool OtpShieldEnabled { get => _otpShieldEnabled; set => SetProperty(ref _otpShieldEnabled, value); }
        private bool _syncEnabled = true;
        public bool SyncEnabled { get => _syncEnabled; set { if (SetProperty(ref _syncEnabled, value)) DaemonClient.SetSyncEnabled(value); } }
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
                if (!IsDaemonRunning) return "#FF3B30";
                if (ConnectedCount > 0) return "#34C759";
                if (AttentionCount > 0) return "#FF9500";
                return "#8E8E93";
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
                        IsDaemonRunning = isRunning;
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
                    ActivityFeed = new ObservableCollection<ActivityEntry>(entries.OrderByDescending(e => e.timestamp_ms));
                    OnPropertyChanged(nameof(ActivityCount));
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
                    PendingClipboards = new ObservableCollection<PendingClipboard>(clips.OrderByDescending(c => c.timestamp_ms));
                    NotifyPendingClipboardMetrics();
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
                if (newPeers != null)
                {
                    Peers = new ObservableCollection<PeerViewModel>(newPeers);
                    var connected = newPeers.Where(p => p.is_trusted && p.status == "connected").ToList();
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
            OnPropertyChanged(nameof(ConnectedCount));
            OnPropertyChanged(nameof(TrustedCount));
            OnPropertyChanged(nameof(AttentionCount));
            OnPropertyChanged(nameof(HeaderStatusText));
            OnPropertyChanged(nameof(HeaderStatusBrush));
        }

        private void NotifyTransferMetrics()
        {
            OnPropertyChanged(nameof(HasActiveTransfers));
            OnPropertyChanged(nameof(HasActiveSpeedTests));
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
                        System.Threading.Tasks.Task.Run(() => DaemonClient.PushFile(targetDeviceId, f.Path));
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
        public bool IsPinned { get => _isPinned; set { if (SetProperty(ref _isPinned, value)) OnPropertyChanged(nameof(PinColor)); } }
        public string PinColor => IsPinned ? "#32ADE6" : "#8E8E93";
        public string TypeIcon { get; set; } = "📝";
        public string Summary { get; set; } = "";
        public string FullText { get; set; } = "";
        public string Source { get; set; } = "";
        public string RelativeTime { get; set; } = "Just now";
        public DateTime Time { get; set; } = DateTime.Now;
        public string Id { get => id; set => id = value; }
    } 
}











