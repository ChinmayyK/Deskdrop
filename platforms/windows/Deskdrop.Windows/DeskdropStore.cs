using System;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Text.Json;
using System.Windows;
using System.Windows.Threading;

namespace Deskdrop.Windows
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
        public string kind { get => _kind; set => SetProperty(ref _kind, value); }
        private string _summary = "";
        public string summary { get => _summary; set => SetProperty(ref _summary, value); }
        private ulong _timestamp;
        public ulong timestamp { get => _timestamp; set => SetProperty(ref _timestamp, value); }
        private string _source = "";
        public string source { get => _source; set => SetProperty(ref _source, value); }
        private string _content_hash = "";
        public string content_hash { get => _content_hash; set => SetProperty(ref _content_hash, value); }
    }

    public class PendingClipboard : BaseViewModel
    {
        private string _content_hash = "";
        public string content_hash { get => _content_hash; set => SetProperty(ref _content_hash, value); }
        private string _summary = "";
        public string summary { get => _summary; set => SetProperty(ref _summary, value); }
        private string _from_device = "";
        public string from_device { get => _from_device; set => SetProperty(ref _from_device, value); }
        private ulong _timestamp;
        public ulong timestamp { get => _timestamp; set => SetProperty(ref _timestamp, value); }
    }

    public class PeerViewModel : BaseViewModel
    {
        private string _device_id = "";
        [System.Text.Json.Serialization.JsonPropertyName("id")]
        public string device_id { get => _device_id; set => SetProperty(ref _device_id, value); }
        private string _friendly_name = "";
        public string friendly_name { get => _friendly_name; set => SetProperty(ref _friendly_name, value); }
        private string _status = "";
        public string status { get => _status; set { if(SetProperty(ref _status, value)) { OnPropertyChanged(nameof(StatusIcon)); OnPropertyChanged(nameof(ShowVerifyButton)); OnPropertyChanged(nameof(ShowDisconnectButton)); OnPropertyChanged(nameof(ShowConnectButton)); } } }
        private bool _is_trusted;
        [System.Text.Json.Serialization.JsonPropertyName("trusted")]
        public bool is_trusted { get => _is_trusted; set { if(SetProperty(ref _is_trusted, value)) { OnPropertyChanged(nameof(ShowVerifyButton)); OnPropertyChanged(nameof(ShowDisconnectButton)); OnPropertyChanged(nameof(ShowConnectButton)); } } }
        
        public string StatusIcon => status == "connected" ? "CheckCircle" : "Circle";
        
        public bool ShowVerifyButton => status == "connected" && !is_trusted;
        public bool ShowDisconnectButton => status == "connected" && is_trusted;
        public bool ShowConnectButton => status != "connected";

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
        public long bytes_total { get => _bytes_total; set => SetProperty(ref _bytes_total, value); }
        private long _bytes_received;
        public long bytes_received { get => _bytes_received; set => SetProperty(ref _bytes_received, value); }
        private int _percent;
        public int percent { get => _percent; set { if(SetProperty(ref _percent, value)) OnPropertyChanged(nameof(PercentText)); } }
        private string _status = "";
        public string status { get => _status; set { if(SetProperty(ref _status, value)) { OnPropertyChanged(nameof(StatusText)); OnPropertyChanged(nameof(ProgressColor)); OnPropertyChanged(nameof(PrimaryVisible)); OnPropertyChanged(nameof(PrimaryIcon)); OnPropertyChanged(nameof(SecondaryVisible)); } } }
        private string? _destination;
        public string? destination { get => _destination; set => SetProperty(ref _destination, value); }

        public string FileName => file_name;
        public int Percent => percent;
        public string PercentText => $"{percent}%";
        public string StatusText => status == "in_progress" ? $"Receiving from {from_device}..." : status;
        public string ProgressColor => status == "completed" ? "#34C759" : (status == "failed" ? "#FF3B30" : "#007AFF");

        public string PrimaryIcon => status == "incoming" ? "PhoneIncoming" : (status == "in_progress" ? "Pause" : (status == "paused" ? "Play" : (status == "completed" ? "CheckCircle" : "RefreshCw")));
        public string PrimaryBackground => status == "incoming" ? "#34C759" : (status == "in_progress" ? "#F2F2F7" : (status == "paused" ? "#007AFF" : (status == "completed" ? "#E5F9E9" : "#F2F2F7")));
        public string PrimaryForeground => status == "incoming" ? "White" : (status == "in_progress" ? "#1C1C1E" : (status == "paused" ? "White" : (status == "completed" ? "#34C759" : "#1C1C1E")));
        public bool PrimaryVisible => true;

        public bool SecondaryVisible => status == "incoming" || status == "in_progress" || status == "paused";
        public string SecondaryIcon => status == "incoming" ? "X" : "X";
        public string SecondaryBackground => "#FF3B30";
        public string SecondaryForeground => "#FFFFFF";
    }

    public class PeerBatteryState
    {
        public string device_id { get; set; } = "";
        public string device_name { get; set; } = "";
        public int level { get; set; }
        public bool charging { get; set; }
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

        private DispatcherTimer? _pollTimer;

        private DeskdropStore()
        {
            Peers = new ObservableCollection<PeerViewModel>();
            History = new ObservableCollection<HistoryItem>();
            ActiveTransfers = new ObservableCollection<FileTransferState>();
            ActivityFeed = new ObservableCollection<ActivityEntry>();
            PendingClipboards = new ObservableCollection<PendingClipboard>();
            
            StartPolling();
        }

        private void StartPolling()
        {
            _pollTimer = new DispatcherTimer
            {
                Interval = TimeSpan.FromSeconds(1)
            };
            _pollTimer.Tick += (s, e) => UpdateStateFromDaemon();
            _pollTimer.Start();
        }



        private ObservableCollection<PeerViewModel> _peers = null!;
        public ObservableCollection<PeerViewModel> Peers
        {
            get => _peers;
            set { _peers = value; OnPropertyChanged(); }
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
            set { _isDaemonRunning = value; OnPropertyChanged(); }
        }

        private string _statusLine = "Starting...";
        public string StatusLine
        {
            get => _statusLine;
            set { _statusLine = value; OnPropertyChanged(); }
        }

        private int _isRefreshInFlight = 0;

        public void UpdateStateFromDaemon()
        {
            if (System.Threading.Interlocked.CompareExchange(ref _isRefreshInFlight, 1, 0) != 0) return;

            System.Threading.Tasks.Task.Run(() =>
            {
                try
                {
                    bool isRunning = DaemonClient.IsDaemonRunning();
                    
                    System.Windows.Application.Current?.Dispatcher.Invoke(() =>
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

                        var activity = DaemonClient.Send(new { cmd = "activity_recent" });
                        if (activity != null && activity.RootElement.TryGetProperty("data", out var actDataElem))
                        {
                            ParseActivityFeed(actDataElem);
                        }

                        var pending = DaemonClient.Send(new { cmd = "pending_remote_clipboards" });
                        if (pending != null && pending.RootElement.TryGetProperty("data", out var pendDataElem))
                        {
                            ParsePendingClipboards(pendDataElem);
                        }
                    }
                }
                catch (Exception ex)
                {
                    // Handle failure gracefully
                    System.Windows.Application.Current?.Dispatcher.Invoke(() =>
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
            if (dataElem.TryGetProperty("entries", out var entriesElem))
            {
                var entries = JsonSerializer.Deserialize<System.Collections.Generic.List<ActivityEntry>>(entriesElem.GetRawText());
                if (entries != null)
                {
                    System.Windows.Application.Current?.Dispatcher.Invoke(() =>
                    {
                        var existing = ActivityFeed.ToList();
                        foreach (var e in entries)
                        {
                            var match = existing.FirstOrDefault(x => x.id == e.id);
                            if (match != null)
                            {
                                match.kind = e.kind;
                                match.summary = e.summary;
                                match.timestamp = e.timestamp;
                                match.source = e.source;
                                match.content_hash = e.content_hash;
                                existing.Remove(match);
                            }
                            else
                            {
                                ActivityFeed.Add(e);
                            }
                        }
                        foreach (var rem in existing) ActivityFeed.Remove(rem);
                    });
                }
            }
        }

        private void ParsePendingClipboards(JsonElement dataElem)
        {
            if (dataElem.TryGetProperty("clipboards", out var clipboardsElem))
            {
                var clips = JsonSerializer.Deserialize<System.Collections.Generic.List<PendingClipboard>>(clipboardsElem.GetRawText());
                if (clips != null)
                {
                    System.Windows.Application.Current?.Dispatcher.Invoke(() =>
                    {
                        var existing = PendingClipboards.ToList();
                        foreach (var c in clips)
                        {
                            var match = existing.FirstOrDefault(x => x.content_hash == c.content_hash);
                            if (match != null)
                            {
                                match.summary = c.summary;
                                match.from_device = c.from_device;
                                match.timestamp = c.timestamp;
                                existing.Remove(match);
                            }
                            else
                            {
                                PendingClipboards.Add(c);
                            }
                        }
                        foreach (var rem in existing) PendingClipboards.Remove(rem);
                    });
                }
            }
        }

        private void ParseDaemonState(JsonElement dataElem)
        {
            System.Windows.Application.Current?.Dispatcher.Invoke(() =>
            {
                if (dataElem.TryGetProperty("peers", out var peersElem))
                {
                    var newPeers = JsonSerializer.Deserialize<System.Collections.Generic.List<PeerViewModel>>(peersElem.GetRawText());
                    
                    if (dataElem.TryGetProperty("peer_batteries", out var batElem))
                    {
                        var batteries = JsonSerializer.Deserialize<System.Collections.Generic.List<PeerBatteryState>>(batElem.GetRawText());
                        if (newPeers != null && batteries != null)
                        {
                            foreach (var peer in newPeers)
                            {
                                var bat = batteries.Find(b => b.device_id == peer.device_id);
                                if (bat != null)
                                {
                                    peer.BatteryLevel = bat.level;
                                    peer.BatteryCharging = bat.charging;
                                }
                            }
                        }
                    }

                    if (newPeers != null)
                    {
                        var existing = Peers.ToList();
                        foreach(var peer in newPeers)
                        {
                            var match = Peers.FirstOrDefault(p => p.device_id == peer.device_id);
                            if (match != null)
                            {
                                match.friendly_name = peer.friendly_name;
                                match.status = peer.status;
                                match.BatteryLevel = peer.BatteryLevel;
                                match.BatteryCharging = peer.BatteryCharging;
                                existing.Remove(match);
                            }
                            else
                            {
                                Peers.Add(peer);
                            }
                        }
                        foreach(var rem in existing) Peers.Remove(rem);
                        
                        StatusLine = Peers.Count == 0 ? "✅ Running — no devices connected" : $"📡 Connected to {Peers.Count(p => p.status == "connected")} devices";
                    }
                }

                if (dataElem.TryGetProperty("active_transfers", out var transfersElem))
                {
                    var transfers = JsonSerializer.Deserialize<System.Collections.Generic.List<FileTransferState>>(transfersElem.GetRawText());
                    if (transfers != null)
                    {
                        var existing = ActiveTransfers.ToList();
                        foreach (var tr in transfers)
                        {
                            var match = ActiveTransfers.FirstOrDefault(t => t.transfer_id == tr.transfer_id);
                            if (match != null)
                            {
                                match.status = tr.status;
                                match.bytes_received = tr.bytes_received;
                                match.bytes_total = tr.bytes_total;
                                match.percent = tr.percent;
                                match.destination = tr.destination;
                                existing.Remove(match);
                            }
                            else
                            {
                                ActiveTransfers.Add(tr);
                            }
                        }
                        foreach(var rem in existing) ActiveTransfers.Remove(rem);
                    }
                }
                else
                {
                    ActiveTransfers.Clear();
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
    }
}
