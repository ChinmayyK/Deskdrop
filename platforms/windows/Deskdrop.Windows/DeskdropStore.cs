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
    public class DeskdropStore : INotifyPropertyChanged
    {
        public static DeskdropStore Shared { get; } = new DeskdropStore();

        private DispatcherTimer? _pollTimer;

        private DeskdropStore()
        {
            Peers = new ObservableCollection<MainWindow.PeerViewModel>();
            History = new ObservableCollection<HistoryItem>();
            ActiveTransfers = new ObservableCollection<MainWindow.FileTransferState>();
            
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

        public event PropertyChangedEventHandler? PropertyChanged;

        protected void OnPropertyChanged([CallerMemberName] string? propertyName = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
        }

        private ObservableCollection<MainWindow.PeerViewModel> _peers;
        public ObservableCollection<MainWindow.PeerViewModel> Peers
        {
            get => _peers;
            set { _peers = value; OnPropertyChanged(); }
        }

        private ObservableCollection<HistoryItem> _history;
        public ObservableCollection<HistoryItem> History
        {
            get => _history;
            set { _history = value; OnPropertyChanged(); }
        }

        private ObservableCollection<MainWindow.FileTransferState> _activeTransfers;
        public ObservableCollection<MainWindow.FileTransferState> ActiveTransfers
        {
            get => _activeTransfers;
            set { _activeTransfers = value; OnPropertyChanged(); }
        }

        private MainWindow.ActiveCallState? _activeCall;
        public MainWindow.ActiveCallState? ActiveCall
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

        public void UpdateStateFromDaemon()
        {
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
            });
        }

        private void ParseDaemonState(JsonElement dataElem)
        {
            System.Windows.Application.Current?.Dispatcher.Invoke(() =>
            {
                if (dataElem.TryGetProperty("peers", out var peersElem))
                {
                    var newPeers = JsonSerializer.Deserialize<System.Collections.Generic.List<MainWindow.PeerViewModel>>(peersElem.GetRawText());
                    
                    if (dataElem.TryGetProperty("peer_batteries", out var batElem))
                    {
                        var batteries = JsonSerializer.Deserialize<System.Collections.Generic.List<MainWindow.PeerBatteryState>>(batElem.GetRawText());
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
                        // In a real app we'd sync this collection gracefully, for now we just clear and add to avoid complete reallocation issues
                        Peers.Clear();
                        foreach(var p in newPeers) Peers.Add(p);
                        
                        StatusLine = Peers.Count == 0 ? "✅ Running — no devices connected" : $"📡 Connected to {Peers.Count(p => p.status == "connected")} devices";
                    }
                }

                if (dataElem.TryGetProperty("active_transfers", out var transfersElem))
                {
                    var transfers = JsonSerializer.Deserialize<System.Collections.Generic.List<MainWindow.FileTransferState>>(transfersElem.GetRawText());
                    if (transfers != null)
                    {
                        ActiveTransfers.Clear();
                        foreach(var t in transfers) ActiveTransfers.Add(t);
                    }
                }
                else
                {
                    ActiveTransfers.Clear();
                }

                if (dataElem.TryGetProperty("active_call", out var callElem) && callElem.ValueKind != JsonValueKind.Null)
                {
                    ActiveCall = JsonSerializer.Deserialize<MainWindow.ActiveCallState>(callElem.GetRawText());
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
