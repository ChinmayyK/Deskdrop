use crate::activity::ActivityFeed;
use crate::dedup::hash_content;
use crate::discovery::{Discovery, PeerEvent, PeerInfo};
use crate::file_transfer::{default_save_dir, FileTransferManager};
use crate::identity::IdentityStore;
use crate::mesh::{ClipboardApplyPolicy, MeshRouter};
use crate::network::{self, PeerSession, Server};
use crate::network_manager::{self, NetworkChangeEvent, NetworkInterfaceInfo};
use crate::peer_manager::{
    DiscoverySource, PeerConnectionState, PeerManager, PeerRecord, SessionShutdown,
};
use crate::probe::{self, ProbeResult, QualityProbe};
use crate::protocol::{
    AppMessage, ClipboardContent, FileTransferMetadata, HistoryMetadata, DEFAULT_PORT,
};
use crate::retry::Backoff;
use crate::settings::{
    default_peer_store_path, default_settings_path, default_trust_store_path, Settings,
    SettingsStore,
};
use crate::trust::{TrustRecord, TrustState, TrustStore};
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::timeout;
use tracing::{error, info, warn};
use uuid::Uuid;

pub(crate) mod clipboard;
pub(crate) mod file_ops;
pub(crate) mod telemetry;
pub(crate) use file_ops::*;

/// RFC 7396 JSON merge-patch: recursively overwrite `target` with non-null
/// fields from `patch`, removing null-keyed fields.
fn json_merge_patch(target: &mut serde_json::Value, patch: &serde_json::Value) {
    if let serde_json::Value::Object(patch_obj) = patch {
        if !target.is_object() {
            *target = serde_json::Value::Object(serde_json::Map::new());
        }
        let target_obj = target
            .as_object_mut()
            .expect("target is explicitly converted to an object");
        for (key, patch_val) in patch_obj {
            if patch_val.is_null() {
                target_obj.remove(key);
            } else if let Some(existing) = target_obj.get_mut(key) {
                json_merge_patch(existing, patch_val);
            } else {
                target_obj.insert(key.clone(), patch_val.clone());
            }
        }
    } else {
        *target = patch.clone();
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum SystemHealthState {
    Ready,
    NoPeers,
    DiscoveryInProgress,
    NeedsLocalNetworkPermission,
    NeedsNotificationsPermission,
    BatteryRestricted,
    DaemonStopped,
    FirewallOrNetworkBlocked,
    TrustPending,
    SyncPaused,
    DeliveryQueued,
    ActionRequired(String),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum DeliveryStatus {
    Queued,
    Sent,
    Delivered,
    Applied,
    Failed(String),
}

#[derive(Debug)]
pub enum EngineEvent {
    /// A remote clipboard item arrived and was added to the activity feed.
    /// If `auto_applied` is true it was also written to the local clipboard.
    ClipboardReceived {
        from_device: Uuid,
        from_name: String,
        content: std::sync::Arc<ClipboardContent>,
        /// True when the engine auto-applied it to the local clipboard.
        /// False when timeline-first mode is active (user must apply manually).
        auto_applied: bool,
        /// Relay path that brought this item here.
        relay_path: Vec<String>,
        /// Activity feed entry ID for this event.
        activity_id: u64,
    },
    HistoryMetadataReceived {
        from_device: Uuid,
        from_name: String,
        entry: HistoryMetadata,
    },
    ClipboardSynced {
        peer_device: Uuid,
        peer_name: String,
        seq: u64,
    },
    ClipboardSyncFailed {
        peer_device: Uuid,
        peer_name: String,
        seq: u64,
        reason: String,
    },
    PairingRequested {
        device_id: Uuid,
        device_name: String,
        pin: String,
    },
    OutgoingPairingWaiting {
        device_id: Uuid,
        device_name: String,
        pin: String,
    },
    PairingConfirmed {
        device_id: Uuid,
    },
    PairingRejected {
        device_id: Uuid,
    },
    /// An untrusted peer was discovered on the network (useful for UI lists).
    PeerDiscovered {
        device_id: Uuid,
        device_name: String,
        platform: Option<String>,
    },
    SystemHealthUpdated(SystemHealthState),
    ClipboardDeliveryStatus {
        activity_id: u64,
        status: DeliveryStatus,
    },
    PeerConnected {
        device_id: Uuid,
        device_name: String,
        addr: SocketAddr,
        trusted: bool,
    },
    PeerDisconnected {
        device_id: Uuid,
        device_name: Option<String>,
        reason: Option<String>,
    },
    /// A remote device wants to send a file — UI should prompt user to accept.
    FileTransferIncoming {
        transfer_id: [u8; 16],
        from_device: Uuid,
        from_name: String,
        file_name: String,
        file_bytes: u64,
        mime_type: String,
    },
    /// File transfer progress update.
    FileTransferProgress {
        transfer_id: [u8; 16],
        from_device: Uuid,
        file_name: String,
        percent: u8,
        bytes_received: u64,
        total_bytes: u64,
        speed_bps: Option<u64>,
        eta_secs: Option<u64>,
    },
    /// File transfer completed and is ready at `dest_path`.
    FileTransferComplete {
        transfer_id: [u8; 16],
        from_device: Uuid,
        from_name: String,
        file_name: String,
        dest_path: PathBuf,
    },
    /// File transfer failed or was cancelled.
    FileTransferFailed {
        transfer_id: [u8; 16],
        from_device: Uuid,
        reason: String,
    },
    /// File transfer was paused.
    FileTransferPaused {
        transfer_id: [u8; 16],
    },
    /// File transfer was resumed.
    FileTransferResumed {
        transfer_id: [u8; 16],
    },

    // ── Speed Test Events ────────────────────────────────────────────────────
    SpeedTestProgress {
        test_id: Uuid,
        peer_id: Uuid,
        direction: String, // "upload" or "download"
        bytes_transferred: u64,
        duration_secs: u32,
    },
    SpeedTestComplete {
        test_id: Uuid,
        peer_id: Uuid,
    },
    /// Activity feed snapshot (full or incremental). Used to update the UI.
    ActivityFeedUpdated {
        entries: Vec<crate::activity::ActivityEntry>,
    },
    /// A connected Android device reported a phone call state change.
    /// Used by macOS to show an incoming-call banner; by Android to update UI.
    CallStateChanged {
        from_device: Uuid,
        from_name: String,
        /// "ringing", "offhook", "idle"
        state: String,
        number: String,
        contact_name: String,
    },
    /// A remote peer requested a call action (accept/decline).
    /// Consumed by the Android JNI layer to invoke TelecomManager APIs.
    CallActionRequest {
        action: String,
        from_device: Uuid,
    },
    /// A connected peer device reported a battery status change (F20).
    BatteryStateChanged {
        from_device: Uuid,
        from_name: String,
        level: u8,
        charging: bool,
    },
    /// A connected peer device reported a network status change.
    NetworkStateChanged {
        from_device: Uuid,
        from_name: String,
        network_type: String,
    },
    /// A connected Android device relayed a push notification.
    NotificationReceived {
        id: String,
        package: String,
        title: String,
        text: String,
        from_device: Uuid,
        from_name: String,
    },
    /// A remote peer requested to start the virtual camera stream.
    CameraStreamRequest {
        from_device: Uuid,
    },
    /// A remote peer accepted or rejected the camera stream request.
    CameraStreamAccept {
        from_device: Uuid,
        accepted: bool,
    },
    /// A remote peer stopped the camera stream.
    CameraStreamStop {
        from_device: Uuid,
    },
    /// A raw video frame was received for the virtual camera stream.
    CameraFrameReceived {
        from_device: Uuid,
    },
    RemoteFilesQueryReceived {
        request_id: Uuid,
        from_device: Uuid,
        summary_only: bool,
        category: Option<crate::protocol::RemoteFileCategory>,
        source: Option<crate::protocol::RemoteFileSource>,
        search_query: Option<String>,
        offset: u32,
        limit: u32,
    },
    RemoteFilesResponseReceived {
        request_id: Uuid,
        from_device: Uuid,
        summary: Option<crate::protocol::RemoteFilesSummary>,
        files: Vec<crate::protocol::RemoteFileEntry>,
        total_matching: u32,
        error: Option<String>,
    },
    RemoteThumbnailRequestReceived {
        request_id: Uuid,
        from_device: Uuid,
        file_id: u64,
        size_px: u32,
    },
    RemoteThumbnailResponseReceived {
        request_id: Uuid,
        from_device: Uuid,
        file_id: u64,
        data: Vec<u8>,
        error: Option<String>,
    },
    RemoteFilePullRequestReceived {
        request_id: Uuid,
        from_device: Uuid,
        file_id: u64,
    },
    RemoteFileActionRequestReceived {
        from_device: Uuid,
        action: String,
        file_id: u64,
        new_name: Option<String>,
    },
    /// An untrusted peer has requested to pair with this device.
    PairingRequest {
        device_id: Uuid,
        device_name: String,
    },
    /// A peer responded to our pairing request.
    PairingResponse {
        device_id: Uuid,
        accepted: bool,
    },
    Warning(String),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncTarget {
    All,
    Device(Uuid),
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncDispatchPeer {
    pub device_id: Uuid,
    pub device_name: String,
    pub delivered: bool,
    pub metadata_only: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncDispatchReport {
    pub seq: u64,
    pub target: SyncTarget,
    pub peers: Vec<SyncDispatchPeer>,
}

impl SyncDispatchReport {
    pub fn delivered_count(&self) -> usize {
        self.peers
            .iter()
            .filter(|peer| peer.delivered && !peer.metadata_only)
            .count()
    }
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub device_id: Uuid,
    pub device_name: String,
    pub port: u16,
    pub trust_store_path: PathBuf,
    pub peer_store_path: PathBuf,
    pub identity_path: PathBuf,
    pub connect_timeout: Duration,
    pub heartbeat_interval: Duration,
    pub heartbeat_timeout: Duration,
    pub bind_ip: Option<IpAddr>,
    pub enable_discovery: bool,
    pub network_poll_interval: Duration,
    /// Root directory for daemon-managed data files (history, feedback, etc.).
    pub data_dir: PathBuf,
    /// Optional override for dedicated file transfer saves.
    pub file_save_dir: Option<PathBuf>,
    /// Maximum number of history entries to keep in memory and on disk.
    pub history_limit: Option<usize>,
}

pub fn default_device_name() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("scutil")
            .args(["--get", "ComputerName"])
            .output()
        {
            let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !name.is_empty() {
                return name;
            }
        }
    }
    whoami::devicename()
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            device_id: Uuid::nil(),
            device_name: default_device_name(),
            port: DEFAULT_PORT,
            trust_store_path: default_trust_store_path(),
            peer_store_path: default_peer_store_path(),
            identity_path: IdentityStore::default_path(),
            connect_timeout: Duration::from_secs(3),
            heartbeat_interval: Duration::from_secs(5),
            heartbeat_timeout: Duration::from_secs(12),
            bind_ip: None,
            enable_discovery: true,
            network_poll_interval: Duration::from_secs(1),
            data_dir: default_peer_store_path()
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".")),
            file_save_dir: None,
            history_limit: Some(500),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EngineStatus {
    pub active_interface: Option<NetworkInterfaceInfo>,
    pub bind_address: SocketAddr,
    pub peers: Vec<PeerRecord>,
    pub last_sync_at: Option<u64>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeNetworkState {
    bind_addr: SocketAddr,
    active_interface: Option<NetworkInterfaceInfo>,
}

#[derive(Debug)]
pub(crate) enum ListenerCommand {
    Rebind(SocketAddr),
}

#[derive(Debug)]
pub(crate) enum DiscoveryCommand {
    Restart { bind_ip: IpAddr, port: u16 },
}

/// Active phone call state tracked by the engine.
/// Updated when a connected Android device reports call state changes.
/// Exposed in the IPC status response so macOS can poll it.
#[derive(Debug, Clone, Serialize)]
pub struct ActiveCallState {
    pub device_id: Uuid,
    pub device_name: String,
    pub state: String,
    pub number: String,
    pub contact_name: String,
}

/// Battery level from a connected peer device (F20).
/// Updated when a BatteryStatus message is received.
#[derive(Debug, Clone, Serialize)]
pub struct PeerBatteryState {
    pub device_id: Uuid,
    pub device_name: String,
    pub level: u8,
    pub charging: bool,
}

/// Network connection state from a connected peer device.
#[derive(Debug, Clone, Serialize)]
pub struct PeerNetworkState {
    pub device_id: Uuid,
    pub device_name: String,
    pub network_type: String,
}

/// Storage state from a connected peer device.
#[derive(Debug, Clone, Serialize)]
pub struct PeerStorageState {
    pub device_id: Uuid,
    pub device_name: String,
    pub images_bytes: u64,
    pub videos_bytes: u64,
    pub apps_bytes: u64,
    pub free_bytes: u64,
    pub total_bytes: u64,
}

/// Result of a remote files query (`query_remote_files_sync`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemoteFilesResult {
    pub summary: Option<crate::protocol::RemoteFilesSummary>,
    pub files: Vec<crate::protocol::RemoteFileEntry>,
    pub total_matching: u32,
    pub error: Option<String>,
}

/// Result of a remote thumbnail request (`request_remote_thumbnail_sync`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemoteThumbnailResult {
    pub file_id: u64,
    pub data: Vec<u8>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub(crate) struct EngineShared {
    pub(crate) config: EngineConfig,
    pub(crate) trust: Arc<Mutex<TrustStore>>,
    pub(crate) peer_manager: Arc<PeerManager>,
    pub(crate) event_tx: mpsc::Sender<EngineEvent>,
    pub(crate) identity_key: Arc<std::sync::RwLock<crate::identity::IdentityKey>>,
    pub(crate) network_state: Arc<Mutex<RuntimeNetworkState>>,
    pub(crate) listener_tx: mpsc::Sender<ListenerCommand>,
    pub(crate) discovery_tx: Option<mpsc::Sender<DiscoveryCommand>>,
    pub(crate) network_reconcile: Arc<Mutex<()>>,
    // ── New: mesh-aware shared state ─────────────────────────────────────────
    /// Mesh fanout router + relay dedup (shared, lock-protected).
    pub(crate) mesh_router: Arc<Mutex<MeshRouter>>,
    /// Cross-device activity feed.
    pub(crate) activity: Arc<Mutex<ActivityFeed>>,
    /// File transfer manager.
    pub(crate) file_transfers: Arc<Mutex<FileTransferManager>>,
    /// Speed tests manager.
    pub(crate) speed_tests:
        Arc<Mutex<std::collections::HashMap<uuid::Uuid, crate::speed_test::SpeedTestState>>>,
    /// Clipboard apply policy (timeline-first vs auto-apply).
    pub(crate) apply_policy: Arc<Mutex<ClipboardApplyPolicy>>,
    /// Settings snapshot for policy decisions (updated lazily).
    pub(crate) settings: Arc<Mutex<Settings>>,
    /// Per-peer link-quality probes — drives adaptive chunk sizing (HIGH-03).
    /// Keyed by peer device UUID; populated on first Pong receipt.
    pub(crate) quality_probes: Arc<Mutex<std::collections::HashMap<uuid::Uuid, QualityProbe>>>,
    /// Clipboard content store — maps content hash → text payload for repush.
    pub(crate) clipboard_store: Arc<Mutex<crate::engine_support::ClipboardStore>>,
    /// Local clipboard reader (platform abstraction for push_current_clipboard).
    pub(crate) local_clipboard: Arc<Mutex<crate::engine_support::LocalClipboard>>,
    /// Persistent history store.
    pub(crate) history: Arc<Mutex<crate::history::History>>,
    /// In-memory feedback event log (most-recent N events).
    pub(crate) feedback: Arc<Mutex<crate::engine_support::FeedbackLog>>,
    /// Tracks when the local device woke up from sleep. Prevents immediate disconnects when waking from deep sleep.
    pub(crate) local_last_wake: Arc<std::sync::atomic::AtomicU64>,
    /// Active phone call state (set on ringing/offhook, cleared on idle).
    pub(crate) active_call: Arc<Mutex<Option<ActiveCallState>>>,
    /// Per-peer battery levels (F20). Keyed by device UUID.
    pub(crate) peer_batteries: Arc<Mutex<std::collections::HashMap<uuid::Uuid, PeerBatteryState>>>,
    pub(crate) peer_storage: Arc<Mutex<std::collections::HashMap<uuid::Uuid, PeerStorageState>>>,
    /// Cache of local battery state to push to newly connected peers.
    pub(crate) local_battery: Arc<Mutex<Option<(u8, bool)>>>,
    /// Cache of local storage state to push to newly connected peers.
    #[allow(clippy::type_complexity)]
    pub(crate) local_storage: Arc<Mutex<Option<(u64, u64, u64, u64, u64)>>>,
    /// Cache of local network state to push to newly connected peers.
    local_network: Arc<Mutex<Option<String>>>,
    /// Per-peer network status. Keyed by device UUID.
    peer_networks: Arc<Mutex<std::collections::HashMap<uuid::Uuid, PeerNetworkState>>>,
    /// Per-peer latest camera frame (to prevent MPSC channel OOM).
    pub camera_frames: Arc<Mutex<std::collections::HashMap<uuid::Uuid, Vec<u8>>>>,
    /// Rate limit for pairing UI spam from untrusted peers.

    /// Throttle for inbound clipboard pushes.
    pub throttle: crate::throttle::Throttle,
    /// AIMD adaptive congestion controller for dynamic bandwidth adjustment.
    pub congestion_controller: crate::throttle::AdaptiveCongestionController,
    /// Cross-device duplicate prevention (mesh echo suppression).
    pub dedup: Arc<Mutex<crate::dedup::Deduplicator>>,
    /// Active QR authentication token (short-lived)
    pub qr_auth_token: Arc<Mutex<Option<String>>>,
    /// Waiters for remote files queries (`query_remote_files_sync`). Keyed by `request_id`.
    pub(crate) remote_file_waiters: Arc<
        Mutex<
            std::collections::HashMap<uuid::Uuid, tokio::sync::oneshot::Sender<RemoteFilesResult>>,
        >,
    >,
    /// Waiters for remote thumbnail requests (`request_remote_thumbnail_sync`). Keyed by `request_id`.
    pub(crate) remote_thumb_waiters: Arc<
        Mutex<
            std::collections::HashMap<
                uuid::Uuid,
                tokio::sync::oneshot::Sender<RemoteThumbnailResult>,
            >,
        >,
    >,
}

#[derive(Clone)]
pub struct Engine {
    pub(crate) shared: EngineShared,
    pub(crate) seq: std::sync::Arc<tokio::sync::Mutex<u64>>,
}

impl Engine {
    pub async fn start(config: EngineConfig, event_tx: mpsc::Sender<EngineEvent>) -> Result<Self> {
        let mut config = config;
        ensure_parent(&config.trust_store_path)?;
        ensure_parent(&config.peer_store_path)?;
        ensure_parent(&config.identity_path)?;

        let identity = IdentityStore::new(&config.identity_path)
            .load_or_create()
            .context("loading identity key")?;
        if config.device_id.is_nil() {
            config.device_id = stable_device_id(identity.public_bytes);
        }
        let trust = Arc::new(Mutex::new(
            TrustStore::load(&config.trust_store_path).context("loading trust store")?,
        ));
        let peer_manager =
            Arc::new(PeerManager::load(&config.peer_store_path).context("loading peer store")?);

        let (active_interface, bind_addr) = resolve_bind_address(&config)?;
        let (listener_tx, listener_rx) = mpsc::channel(8);
        let discovery_pair = if config.enable_discovery {
            let (tx, rx) = mpsc::channel(8);
            Some((tx, rx))
        } else {
            None
        };

        let shared = EngineShared {
            config: config.clone(),
            trust,
            peer_manager,
            event_tx: event_tx.clone(),
            identity_key: Arc::new(std::sync::RwLock::new(identity)),
            network_state: Arc::new(Mutex::new(RuntimeNetworkState {
                bind_addr,
                active_interface,
            })),
            listener_tx: listener_tx.clone(),
            discovery_tx: discovery_pair.as_ref().map(|(tx, _)| tx.clone()),
            network_reconcile: Arc::new(Mutex::new(())),
            mesh_router: Arc::new(Mutex::new(MeshRouter::new(
                config.device_id,
                config.device_name.clone(),
            ))),
            activity: Arc::new(Mutex::new(ActivityFeed::new(200))),
            file_transfers: Arc::new(Mutex::new(FileTransferManager::new(
                config
                    .file_save_dir
                    .clone()
                    .unwrap_or_else(default_save_dir),
            ))),
            speed_tests: Arc::new(Mutex::new(std::collections::HashMap::new())),
            apply_policy: Arc::new(Mutex::new(ClipboardApplyPolicy::default())),
            settings: Arc::new(Mutex::new(Settings::default())),
            quality_probes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            clipboard_store: Arc::new(Mutex::new(crate::engine_support::ClipboardStore::default())),
            local_clipboard: Arc::new(Mutex::new(crate::engine_support::LocalClipboard::new())),
            history: Arc::new(Mutex::new({
                let history_path = config.data_dir.join("history.json");
                let limit = config.history_limit.unwrap_or(500);
                crate::history::History::load_with_limit(&history_path, limit).unwrap_or_else(
                    |_| {
                        // If the history file is missing or corrupt, start fresh
                        // in a temp path so the daemon always starts successfully.
                        let tmp = std::env::temp_dir().join("deskdrop_history_fallback.json");
                        crate::history::History::load_with_limit(&tmp, limit)
                            .expect("cannot create fallback history store")
                    },
                )
            })),
            feedback: Arc::new(Mutex::new(crate::engine_support::FeedbackLog::new(200))),
            local_last_wake: Arc::new(std::sync::atomic::AtomicU64::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
            )),
            active_call: Arc::new(Mutex::new(None)),
            peer_batteries: Arc::new(Mutex::new(std::collections::HashMap::new())),
            peer_storage: Arc::new(Mutex::new(std::collections::HashMap::new())),
            local_battery: Arc::new(Mutex::new(None)),
            local_storage: Arc::new(Mutex::new(None)),
            local_network: Arc::new(Mutex::new(None)),
            peer_networks: Arc::new(Mutex::new(std::collections::HashMap::new())),
            camera_frames: Arc::new(Mutex::new(std::collections::HashMap::new())),

            throttle: crate::throttle::Throttle::default_rate(),
            congestion_controller: crate::throttle::AdaptiveCongestionController::default(),
            dedup: Arc::new(Mutex::new(crate::dedup::Deduplicator::new())),
            qr_auth_token: Arc::new(Mutex::new(None)),
            remote_file_waiters: Arc::new(Mutex::new(std::collections::HashMap::new())),
            remote_thumb_waiters: Arc::new(Mutex::new(std::collections::HashMap::new())),
        };

        spawn_listener_supervisor(shared.clone(), listener_rx);
        if let Some((_, discovery_rx)) = discovery_pair {
            spawn_discovery_supervisor(shared.clone(), discovery_rx);
        }

        let engine = Self {
            shared: shared.clone(),
            seq: Arc::new(Mutex::new(0)),
        };

        let initial_bind = {
            let state = engine.shared.network_state.lock().await;
            state.bind_addr
        };
        send_listener_rebind(&engine.shared, initial_bind).await?;
        let discovery_ip = {
            let state = engine.shared.network_state.lock().await;
            state
                .active_interface
                .as_ref()
                .map(|i| i.ip)
                .unwrap_or(initial_bind.ip())
        };
        let _identity_pubkey = shared
            .identity_key
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .public_bytes;

        if let Some(discovery_tx) = &engine.shared.discovery_tx {
            if discovery_ip.is_unspecified() {
                // Network interface not ready yet at startup — spawn a background
                // task that retries until a real IP is available, then kicks mDNS.
                // Without this, discovery silently never starts and the user has
                // to manually click "Scan" to trigger it.
                let retry_shared = engine.shared.clone();
                let retry_tx = discovery_tx.clone();
                tokio::spawn(async move {
                    for attempt in 1..=20 {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        let ip = {
                            let state = retry_shared.network_state.lock().await;
                            state
                                .active_interface
                                .as_ref()
                                .map(|i| i.ip)
                                .unwrap_or(state.bind_addr.ip())
                        };
                        if !ip.is_unspecified() {
                            tracing::info!(
                                "discovery retry #{}: network ready at {}, starting mDNS",
                                attempt,
                                ip
                            );
                            let _ = retry_tx
                                .send(DiscoveryCommand::Restart {
                                    bind_ip: ip,
                                    port: retry_shared.config.port,
                                })
                                .await;
                            break;
                        }
                        tracing::debug!(
                            "discovery retry #{}: network still not ready, waiting...",
                            attempt
                        );
                    }
                });
            } else {
                let _ = discovery_tx
                    .send(DiscoveryCommand::Restart {
                        bind_ip: discovery_ip,
                        port: engine.shared.config.port,
                    })
                    .await;
            }
        }

        engine.spawn_network_monitor().await?;
        engine.spawn_peer_pruner();
        engine.spawn_sensitive_history_pruner();
        engine.spawn_auto_reconnector();

        // Spawn UDP broadcast beacon and listener for resilient discovery
        engine.spawn_udp_beacon();
        engine.spawn_udp_listener();

        Ok(engine)
    }

    fn spawn_udp_beacon(&self) {
        let shared = self.shared.clone();
        tokio::spawn(async move {
            let socket = match tokio::net::UdpSocket::bind("0.0.0.0:0").await {
                Ok(s) => s,
                Err(err) => {
                    tracing::warn!(error = %err, "failed to bind UDP beacon socket");
                    return;
                }
            };
            if let Err(err) = socket.set_broadcast(true) {
                tracing::warn!(error = %err, "failed to set broadcast flag on UDP beacon socket");
            }

            // TRU-06: Do NOT include device_name in the beacon — only opaque UUIDs
            // are broadcast. The friendly device name is exchanged only after a
            // successful encrypted handshake via HelloFrame/HelloAck.
            // Format: DESKDROP_BEACON:<uuid>:<tcp_port>:<protocol_version>
            let payload = format!(
                "DESKDROP_BEACON:{}:{}:{}",
                shared.config.device_id,
                shared.config.port,
                crate::protocol::PROTOCOL_VERSION
            )
            .into_bytes();

            let broadcast_addr: SocketAddr =
                "255.255.255.255:47824".parse().expect("static IP is valid");

            // ── AirDrop-style startup burst ──────────────────────────────────
            // Send 3 rapid beacons in the first 300ms so peers discover us
            // almost instantly, then fall back to the regular interval.
            for _ in 0..3 {
                let _ = socket.send_to(&payload, broadcast_addr).await;
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }

            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(1500));
            loop {
                interval.tick().await;
                // Send to limited broadcast address.
                if let Err(err) = socket.send_to(&payload, broadcast_addr).await {
                    tracing::trace!(error = %err, "failed to send UDP beacon");
                }
                // Also send to subnet-directed broadcast addresses for better
                // delivery on networks that filter limited broadcast.
                if let Ok(ifaces) = if_addrs::get_if_addrs() {
                    for iface in ifaces {
                        if iface.is_loopback() {
                            continue;
                        }
                        if let if_addrs::IfAddr::V4(v4) = &iface.addr {
                            if let Some(bcast) = v4.broadcast {
                                let dest = SocketAddr::new(std::net::IpAddr::V4(bcast), 47824);
                                if dest != broadcast_addr {
                                    let _ = socket.send_to(&payload, dest).await;
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    fn spawn_udp_listener(&self) {
        let shared = self.shared.clone();
        tokio::spawn(async move {
            let socket = match tokio::net::UdpSocket::bind("0.0.0.0:47824").await {
                Ok(s) => s,
                Err(err) => {
                    tracing::warn!(error = %err, "failed to bind UDP listener socket on port 47824");
                    return;
                }
            };
            let socket = Arc::new(socket);
            let mut buf = vec![0u8; 1024];
            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((len, addr)) => {
                        let text = String::from_utf8_lossy(&buf[..len]);

                        // ── Handle CONNECTBACK requests ──────────────────────
                        // A peer whose outbound TCP failed is asking US to
                        // initiate the connection to THEM instead.
                        if text.starts_with("DESKDROP_CONNECTBACK:") {
                            let parts: Vec<&str> = text.splitn(4, ':').collect();
                            if parts.len() < 3 {
                                continue;
                            }
                            let peer_id = match uuid::Uuid::parse_str(parts[1]) {
                                Ok(id) => id,
                                Err(_) => continue,
                            };
                            if peer_id == shared.config.device_id {
                                continue;
                            }
                            let peer_port = match parts[2].parse::<u16>() {
                                Ok(p) => p,
                                Err(_) => continue,
                            };
                            let peer_addr = SocketAddr::new(addr.ip(), peer_port);

                            // Skip if already connected to this peer.
                            if shared.peer_manager.is_connected(peer_id) {
                                continue;
                            }

                            tracing::info!(
                                "UDP CONNECTBACK: peer {} at {} is asking us to connect to them",
                                peer_id,
                                peer_addr
                            );
                            let shared_clone = shared.clone();
                            tokio::spawn(async move {
                                if let Err(err) = connect_once(
                                    shared_clone,
                                    vec![peer_addr],
                                    Some(peer_id),
                                    DiscoverySource::UdpBeacon,
                                    false,
                                )
                                .await
                                {
                                    tracing::warn!(
                                        peer_id = %peer_id,
                                        error = %err,
                                        "CONNECTBACK connection attempt failed"
                                    );
                                }
                            });
                            continue;
                        }

                        if !text.starts_with("DESKDROP_BEACON:") {
                            continue;
                        }
                        let parts: Vec<&str> = text.splitn(5, ':').collect();
                        // Accept both new format (4 fields: magic:uuid:port:version)
                        // and legacy format (4 fields: magic:uuid:port:name).
                        if parts.len() < 3 {
                            continue;
                        }
                        let peer_id = match uuid::Uuid::parse_str(parts[1]) {
                            Ok(id) => id,
                            Err(_) => continue,
                        };
                        if peer_id == shared.config.device_id {
                            continue;
                        }
                        let peer_port = match parts[2].parse::<u16>() {
                            Ok(p) => p,
                            Err(_) => continue,
                        };
                        // Protocol version check: if the 4th field parses as a
                        // small integer, treat it as a version. Otherwise, treat
                        // it as a legacy device name (backward compatibility).
                        let peer_name;
                        if parts.len() >= 4 {
                            if let Ok(version) = parts[3].parse::<u16>() {
                                // New format — check protocol compatibility.
                                if version != crate::protocol::PROTOCOL_VERSION {
                                    tracing::debug!(
                                        "UDP beacon: skipping peer {} with protocol v{} (we speak v{})",
                                        peer_id, version, crate::protocol::PROTOCOL_VERSION
                                    );
                                    continue;
                                }
                                peer_name = format!("device-{}", &peer_id.to_string()[..8]);
                            } else {
                                // Legacy format — 4th field is device name.
                                peer_name = parts[3].to_string();
                            }
                        } else {
                            peer_name = format!("device-{}", &peer_id.to_string()[..8]);
                        }

                        let peer_addr = SocketAddr::new(addr.ip(), peer_port);

                        let trusted = {
                            let trust_guard = shared.trust.lock().await;
                            trust_guard.is_trusted(peer_id)
                        };

                        if let Err(err) = shared.peer_manager.upsert_peer(
                            peer_id,
                            peer_name.clone(),
                            peer_addr,
                            trusted,
                            DiscoverySource::UdpBeacon,
                        ) {
                            tracing::warn!(error = %err, "failed to upsert UDP beacon peer");
                        } else {
                            if !should_initiate_session(
                                &shared,
                                peer_id,
                                DiscoverySource::UdpBeacon,
                            )
                            .await
                            {
                                continue;
                            }
                            if shared.peer_manager.live_endpoint(peer_id) == Some(peer_addr) {
                                continue;
                            }
                            if matches!(
                                shared.peer_manager.get(peer_id),
                                Some(record) if record.status == PeerConnectionState::Connecting && record.socket_addrs().contains(&peer_addr)
                            ) {
                                continue;
                            }

                            tracing::info!(
                                "UDP Beacon discovered peer {} at {}",
                                peer_id,
                                peer_addr
                            );

                            // ── Fast Connect-Back ────────────────────────────
                            // Send UDP CONNECTBACK immediately so the remote peer
                            // can connect to us concurrently with our outbound
                            // TCP connect attempt. This eliminates delay when
                            // asymmetric routing / AP isolation blocks outbound TCP.
                            if !shared.peer_manager.is_connected(peer_id) {
                                let connectback = format!(
                                    "DESKDROP_CONNECTBACK:{}:{}",
                                    shared.config.device_id, shared.config.port,
                                );
                                let target = SocketAddr::new(
                                    addr.ip(),
                                    47824, // UDP beacon port
                                );
                                let _ = socket.send_to(connectback.as_bytes(), target).await;
                            }

                            let shared_clone = shared.clone();
                            let socket_clone = socket.clone();
                            let beacon_source_addr = addr;
                            tokio::spawn(async move {
                                if let Err(err) = connect_loop(
                                    shared_clone.clone(),
                                    vec![peer_addr],
                                    Some(peer_id),
                                    DiscoverySource::UdpBeacon,
                                )
                                .await
                                {
                                    tracing::warn!(peer_id = %peer_id, error = %err, "UDP beacon peer connection failed");

                                    if !shared_clone.peer_manager.is_connected(peer_id) {
                                        let connectback = format!(
                                            "DESKDROP_CONNECTBACK:{}:{}",
                                            shared_clone.config.device_id, shared_clone.config.port,
                                        );
                                        let target =
                                            SocketAddr::new(beacon_source_addr.ip(), 47824);
                                        let _ = socket_clone
                                            .send_to(connectback.as_bytes(), target)
                                            .await;
                                    }
                                }
                            });
                        }
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "UDP listener recv_from failed");
                        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    }
                }
            }
        });
    }

    // ── Call Continuity ───────────────────────────────────────────────────────

    // ── F20: Battery synchronization ──────────────────────────────────────────

    // ── Activity Feed ─────────────────────────────────────────────────────────

    // ── Settings ──────────────────────────────────────────────────────────────

    /// Apply new settings to the engine at runtime (no restart needed).
    pub async fn apply_settings(&self, new_settings: Settings) {
        let mut policy = self.shared.apply_policy.lock().await;
        policy.update_from_settings(&new_settings);
        *self.shared.settings.lock().await = new_settings;
    }

    pub async fn current_settings(&self) -> Settings {
        self.shared.settings.lock().await.clone()
    }

    fn persist_settings_snapshot(&self, settings: Settings) -> Result<Settings> {
        let sanitized = settings.sanitize();
        let mut store = SettingsStore::load(default_settings_path())?;
        *store.get_mut() = sanitized.clone();
        store.save()?;
        Ok(sanitized)
    }

    /// Apply a JSON merge-patch to the current settings.
    pub async fn patch_settings(&self, patch: String) -> Result<()> {
        let mut current = serde_json::to_value(&*self.shared.settings.lock().await)?;
        let patch_val: serde_json::Value =
            serde_json::from_str(&patch).context("patch_settings: invalid JSON patch")?;
        json_merge_patch(&mut current, &patch_val);
        let new_settings: Settings = serde_json::from_value(current)
            .context("patch_settings: patched value is invalid Settings")?;
        let persisted = self.persist_settings_snapshot(new_settings)?;
        self.apply_settings(persisted).await;
        Ok(())
    }

    /// Apply a partial settings update from the Mac preferences UI.
    pub async fn save_settings_partial(&self, p: crate::ipc::PartialSettings) -> Result<()> {
        // Clone first so we're not holding the lock while calling apply_settings.
        let mut s = self.shared.settings.lock().await.clone();
        if let Some(v) = p.port {
            s.port = v;
        }
        if let Some(v) = p.device_name {
            s.device_name = v;
        }
        if let Some(v) = p.sync_enabled {
            s.sync_enabled = v;
        }
        if let Some(v) = p.sync_text {
            s.sync_text = v;
        }
        if let Some(v) = p.sync_images {
            s.sync_images = v;
        }
        if let Some(v) = p.sync_files {
            s.sync_files = v;
        }
        if let Some(v) = p.history_limit {
            s.history_limit = v;
        }
        if let Some(v) = p.max_history_text_bytes {
            s.max_history_text_bytes = v;
        }
        if let Some(v) = p.max_payload_bytes {
            s.max_payload_bytes = v;
        }
        if let Some(v) = p.clipboard_poll_ms {
            s.clipboard_poll_ms = v;
        }
        if let Some(v) = p.max_pushes_per_sec {
            s.max_pushes_per_sec = v;
        }
        if let Some(v) = p.rate_limit_burst {
            s.rate_limit_burst = v;
        }
        if let Some(v) = p.smart_sync_duplicate_window_ms {
            s.smart_sync_duplicate_window_ms = v;
        }
        if let Some(v) = p.smart_sync_debounce_ms {
            s.smart_sync_debounce_ms = v;
        }
        if let Some(v) = p.block_sensitive_text {
            s.block_sensitive_text = v;
        }
        if let Some(v) = p.require_tofu_confirmation {
            s.require_tofu_confirmation = v;
        }
        if let Some(v) = p.show_receive_notification {
            s.show_receive_notification = v;
        }
        if let Some(v) = p.ignore_patterns {
            s.ignore_patterns = v;
        }
        let persisted = self.persist_settings_snapshot(s)?;
        self.apply_settings(persisted).await;
        Ok(())
    }

    pub async fn rotate_identity_key(&self) -> Result<()> {
        let new_key = {
            let mut key_lock = self
                .shared
                .identity_key
                .write()
                .unwrap_or_else(|e| e.into_inner());
            let store = crate::identity::IdentityStore::new(&self.shared.config.identity_path);
            *key_lock = store.rotate()?;
            key_lock.public_bytes
        };

        // Broadcast to all active connected peers
        let peers = self.shared.peer_manager.active_senders();
        for (_peer_id, tx) in peers {
            let _ = tx
                .send(crate::protocol::AppMessage::KeyRotated {
                    new_pubkey_bytes: new_key,
                })
                .await;
        }
        Ok(())
    }

    pub async fn set_sync_enabled(&self, enabled: bool) -> Result<()> {
        let mut settings = self.shared.settings.lock().await.clone();
        settings.sync_enabled = enabled;
        let persisted = self.persist_settings_snapshot(settings)?;
        self.apply_settings(persisted).await;
        Ok(())
    }

    pub async fn set_timeline_first_mode(&self, enabled: bool) -> Result<()> {
        let mut settings = self.shared.settings.lock().await.clone();
        settings.timeline_first_mode = enabled;
        let persisted = self.persist_settings_snapshot(settings)?;
        self.apply_settings(persisted).await;
        Ok(())
    }

    pub async fn set_auto_apply_clipboard(&self, enabled: bool) -> Result<()> {
        let mut settings = self.shared.settings.lock().await.clone();
        settings.auto_apply_remote_clipboard = enabled;
        let persisted = self.persist_settings_snapshot(settings)?;
        self.apply_settings(persisted).await;
        Ok(())
    }

    // ── History ───────────────────────────────────────────────────────────────

    /// Record a local text entry in history without syncing it to peers.
    pub async fn remember_text(&self, text: String) -> Result<()> {
        let device_name = self.shared.config.device_name.clone();
        let max_bytes = self.shared.settings.lock().await.max_history_text_bytes;
        let content = crate::protocol::ClipboardContent::Text(text);
        self.shared
            .history
            .lock()
            .await
            .push_with_options(&content, device_name, max_bytes)?;
        Ok(())
    }

    /// Return the raw clipboard content for a pending incoming item by ID.
    pub async fn incoming_clipboard(&self, id: u64) -> Option<serde_json::Value> {
        let entries: Vec<_> = self
            .shared
            .activity
            .lock()
            .await
            .pending_remote_clipboards()
            .into_iter()
            .cloned()
            .collect();
        entries
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| serde_json::to_value(e).ok())
    }

    // ── Templates ─────────────────────────────────────────────────────────────

    // ── Per-peer settings ─────────────────────────────────────────────────────

    pub async fn get_peer_settings(
        &self,
        device_id: Uuid,
    ) -> Option<crate::settings::PeerSettings> {
        self.shared
            .settings
            .lock()
            .await
            .per_peer
            .get(&device_id.to_string())
            .cloned()
    }

    pub async fn patch_peer_settings(&self, device_id: Uuid, patch: String) -> Result<()> {
        let mut settings = self.shared.settings.lock().await;
        let key = device_id.to_string();
        let existing = settings.per_peer.entry(key).or_default();
        let mut val = serde_json::to_value(&*existing)?;
        let patch_val: serde_json::Value =
            serde_json::from_str(&patch).context("patch_peer_settings: invalid JSON patch")?;
        json_merge_patch(&mut val, &patch_val);
        *existing = serde_json::from_value(val)
            .context("patch_peer_settings: patched value is invalid PeerSettings")?;
        Ok(())
    }

    /// Get detailed info for a trusted device.
    pub async fn device_details(&self, device_id: Uuid) -> Option<serde_json::Value> {
        let trust = self.shared.trust.lock().await;
        let record = trust.get(device_id)?;
        serde_json::to_value(record).ok()
    }

    // ── Speed Test ────────────────────────────────────────────────────────────

    pub async fn start_speed_test(&self, device_id: Uuid, duration_secs: u32) -> Result<()> {
        let test_id = Uuid::new_v4();
        if let Some(tx) = self.shared.peer_manager.file_sender(device_id) {
            {
                let mut tests = self.shared.speed_tests.lock().await;
                let entry = tests
                    .entry(device_id)
                    .or_insert_with(|| crate::speed_test::SpeedTestState::new(tx.clone()));
                // We'll mark it as Idle but store the test_id and duration so the response matches
                entry.test_id = Some(test_id);
                entry.duration_secs = duration_secs;
            }
            let req = AppMessage::SpeedTestRequest {
                test_id,
                duration_secs,
            };
            tx.send(req)
                .await
                .map_err(|_| anyhow::anyhow!("Failed to send speed test request"))?;
            Ok(())
        } else {
            anyhow::bail!("Peer not connected");
        }
    }

    // ── Feedback ──────────────────────────────────────────────────────────────

    pub fn set_pairing_requested(&self, device_id: Uuid, requested: bool) -> Result<()> {
        let _ = self
            .shared
            .peer_manager
            .set_pairing_requested(device_id, requested)?;
        Ok(())
    }

    pub fn set_outgoing_pairing_waiting(&self, device_id: Uuid, waiting: bool) -> Result<()> {
        let _ = self
            .shared
            .peer_manager
            .set_outgoing_pairing_waiting(device_id, waiting)?;
        Ok(())
    }

    pub async fn feedback_recent(&self, n: usize) -> Vec<crate::engine_support::FeedbackEvent> {
        self.shared.feedback.lock().await.recent(n)
    }

    /// Send a file to a specific peer (or all if `target_device` is None).
    pub async fn send_file(
        &self,
        data: Vec<u8>,
        file_name: String,
        mime_type: String,
        target_device: Option<Uuid>,
    ) -> Result<[u8; 16]> {
        let mut mgr = self.shared.file_transfers.lock().await;
        let transfer = mgr.start_outbound(data, file_name.clone(), mime_type, target_device)?;
        let transfer_id = transfer.transfer_id;
        let meta = transfer.meta.clone();
        let size_bytes = meta.size_bytes;
        let _ = transfer;
        drop(mgr);

        self.announce_outbound_file_transfer(meta, file_name, size_bytes, target_device)
            .await?;
        Ok(transfer_id)
    }

    /// Send a file from disk without reading the full payload into memory first.
    pub async fn send_file_path(
        &self,
        path: PathBuf,
        file_name: String,
        mime_type: String,
        target_device: Option<Uuid>,
    ) -> Result<[u8; 16]> {
        let mut mgr = self.shared.file_transfers.lock().await;
        let transfer =
            mgr.start_outbound_path(path, file_name.clone(), mime_type, target_device)?;
        let transfer_id = transfer.transfer_id;
        let meta = transfer.meta.clone();
        let size_bytes = meta.size_bytes;
        let _ = transfer;
        drop(mgr);

        self.announce_outbound_file_transfer(meta, file_name, size_bytes, target_device)
            .await?;
        Ok(transfer_id)
    }

    /// Accept an incoming file transfer.
    pub async fn accept_file_transfer(&self, transfer_id: [u8; 16]) -> Result<()> {
        let resume_from = {
            let mut mgr = self.shared.file_transfers.lock().await;
            mgr.accept_inbound_or_resume(&transfer_id)?
        };
        // Find which peer sent this transfer and reply.
        let from_device = {
            let mgr = self.shared.file_transfers.lock().await;
            mgr.all_inbound()
                .iter()
                .find(|t| t.transfer_id == transfer_id)
                .map(|t| t.from_device)
        };
        if let Some(from_device) = from_device {
            let accept_msg = AppMessage::FileTransferAccept {
                transfer_id,
                accepted: true,
                resume_from_chunk: resume_from,
                reject_reason: None,
            };
            let peers = self.shared.peer_manager.all_trusted_senders();
            for (peer_id, tx) in peers {
                if peer_id == from_device {
                    let _ = tx.try_send(accept_msg);
                    break;
                }
            }
        }
        Ok(())
    }

    async fn announce_outbound_file_transfer(
        &self,
        meta: FileTransferMetadata,
        file_name: String,
        size_bytes: u64,
        target_device: Option<Uuid>,
    ) -> Result<()> {
        let transfer_id = meta.transfer_id;
        let announce = AppMessage::FileTransferAnnounce { meta };
        let peers = self.shared.peer_manager.all_connected_senders();
        let mut announced_to = 0usize;
        for (peer_id, tx) in peers {
            let should_send = match target_device {
                Some(t) => t == peer_id,
                None => true,
            };
            if !should_send {
                continue;
            }

            let msg = announce.clone();
            let send_result = match tx.try_send(msg.clone()) {
                Ok(()) => Ok(()),
                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => tx.send(msg).await,
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                    Err(tokio::sync::mpsc::error::SendError(msg))
                }
            };

            if send_result.is_ok() {
                announced_to += 1;
            } else {
                warn!(
                    "file transfer announce queue unavailable for peer {}",
                    peer_id
                );
            }
        }

        if announced_to == 0 {
            self.shared
                .file_transfers
                .lock()
                .await
                .cancel_outbound(&transfer_id);
            return Err(anyhow!("target peer queue unavailable"));
        }

        self.shared
            .activity
            .lock()
            .await
            .record_file_transfer_started(
                self.shared.config.device_id,
                self.shared.config.device_name.clone(),
                file_name,
                size_bytes,
                hex::encode(transfer_id),
                true,
            );
        Ok(())
    }

    /// Reject an incoming file transfer.
    pub async fn reject_file_transfer(&self, transfer_id: [u8; 16], reason: String) -> Result<()> {
        let from_device = {
            let mut mgr = self.shared.file_transfers.lock().await;
            let dev = mgr
                .all_inbound()
                .iter()
                .find(|t| t.transfer_id == transfer_id)
                .map(|t| t.from_device);
            mgr.reject_inbound(&transfer_id);
            dev
        };
        if let Some(from_device) = from_device {
            let reject_msg = AppMessage::FileTransferAccept {
                transfer_id,
                accepted: false,
                resume_from_chunk: 0,
                reject_reason: Some(reason),
            };
            let peers = self.shared.peer_manager.all_trusted_senders();
            for (peer_id, tx) in peers {
                if peer_id == from_device {
                    let _ = tx.try_send(reject_msg);
                    break;
                }
            }
        }
        Ok(())
    }

    /// Cancel an active file transfer (inbound or outbound).
    pub async fn cancel_file_transfer(&self, transfer_id: [u8; 16]) -> Result<()> {
        let cancel_msg = AppMessage::FileTransferCancel {
            transfer_id,
            reason: "user cancelled".into(),
        };
        // Cancel in manager.
        {
            let mut mgr = self.shared.file_transfers.lock().await;
            mgr.cancel_inbound(&transfer_id, "user cancelled");
            mgr.cancel_outbound(&transfer_id);
        }
        // Notify all peers.
        let peers = self.shared.peer_manager.all_trusted_senders();
        for (_, tx) in peers {
            let msg = cancel_msg.clone();
            tokio::spawn(async move {
                let _ = tx.send(msg).await;
            });
        }
        let _ = self
            .shared
            .event_tx
            .send(EngineEvent::FileTransferFailed {
                transfer_id,
                from_device: Uuid::nil(),
                reason: "User cancelled".to_string(),
            })
            .await;
        Ok(())
    }

    /// Pause an active file transfer.
    pub async fn pause_file_transfer(&self, transfer_id: [u8; 16]) -> Result<()> {
        let pause_msg = AppMessage::FileTransferPause { transfer_id };
        {
            let mut mgr = self.shared.file_transfers.lock().await;
            if let Some(t) = mgr.get_outbound_mut(&transfer_id) {
                t.paused = true;
            } else if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                t.paused = true;
            }
        }
        let peers = self.shared.peer_manager.all_trusted_senders();
        for (_, tx) in peers {
            let msg = pause_msg.clone();
            tokio::spawn(async move {
                let _ = tx.send(msg).await;
            });
        }
        let _ = self
            .shared
            .event_tx
            .send(EngineEvent::FileTransferPaused { transfer_id })
            .await;
        Ok(())
    }

    /// Resume a paused file transfer.
    pub async fn resume_file_transfer(&self, transfer_id: [u8; 16]) -> Result<()> {
        let resume_msg = AppMessage::FileTransferResume { transfer_id };
        let mut was_outbound = false;
        let mut target_device = None;
        {
            let mut mgr = self.shared.file_transfers.lock().await;
            if let Some(t) = mgr.get_outbound_mut(&transfer_id) {
                t.paused = false;
                was_outbound = true;
                target_device = t.target_device;
            } else if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                t.paused = false;
            }
        }
        let peers = self.shared.peer_manager.all_trusted_senders();
        for (peer_id, tx) in peers {
            let msg = resume_msg.clone();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                let _ = tx_clone.send(msg).await;
            });

            // If we are the sender, we need to restart the chunking loop!
            // `tx` is the `session_outbox_tx` for this peer.
            if was_outbound && target_device.map(|td| td == peer_id).unwrap_or(true) {
                let bg_outbox = self
                    .shared
                    .peer_manager
                    .file_sender(peer_id)
                    .unwrap_or(tx.clone());
                let bg_shared = self.shared.clone();
                let bg_event_tx = self.shared.event_tx.clone();
                let bg_transfer_id = transfer_id;
                let bg_peer_id = peer_id;
                let mut bg_last_prog_emit: std::collections::HashMap<[u8; 16], std::time::Instant> = std::collections::HashMap::new();
                            tokio::spawn(async move {
                    const BATCH_SIZE: usize = 16;
                    'outer: loop {
                        let (next_chunk, last_acked, total_chunks): (u32, u32, u32) = {
                            let mut mgr = bg_shared.file_transfers.lock().await;
                            if let Some(t) = mgr.get_outbound_mut(&bg_transfer_id) {
                                (t.next_chunk, t.last_acked_chunk.unwrap_or(0), t.total_chunks)
                            } else {
                                break 'outer;
                            }
                        };
                        if next_chunk >= total_chunks {
                            break 'outer;
                        }
                        if next_chunk > 0 && next_chunk.saturating_sub(last_acked) > 512u32 {
                            tokio::time::sleep(std::time::Duration::from_millis(15)).await;
                            continue;
                        }
                        let (batch, progs) = match read_outbound_chunks(
                            bg_shared.clone(),
                            bg_transfer_id,
                            BATCH_SIZE,
                        )
                        .await
                        {
                            Some((batch, progs)) => (batch, progs),
                            None => break 'outer,
                        };

                        
                                    if let Some((prog, fname)) = progs.last() {
                                        let now = std::time::Instant::now();
                                        let last = bg_last_prog_emit.get(&bg_transfer_id).copied().unwrap_or_else(|| now.checked_sub(std::time::Duration::from_secs(1)).unwrap());
                                        if now.duration_since(last).as_millis() >= 100 || prog.percent == 100 {
                                            bg_last_prog_emit.insert(bg_transfer_id, now);
                                            let _ = bg_event_tx
                                                .send(EngineEvent::FileTransferProgress {
                                                    transfer_id: bg_transfer_id,
                                                    from_device: bg_peer_id,
                                                    file_name: fname.clone(),
                                                    percent: prog.percent,
                                                    bytes_received: prog.bytes_received,
                                                    total_bytes: prog.total_bytes,
                                                    speed_bps: prog.speed_bps,
                                                    eta_secs: prog.eta_secs,
                                                })
                                                .await;
                                        }
                                    }

                        if batch.is_empty() {
                            break;
                        }
                        for wire_msg in batch {
                            if bg_outbox.send(wire_msg).await.is_err() {
                                break 'outer;
                            }
                        }
                    }

                    let final_checksum = {
                        let mut mgr = bg_shared.file_transfers.lock().await;
                        mgr.get_outbound_mut(&bg_transfer_id).and_then(|transfer| {
                            if transfer.is_all_sent() {
                                Some(transfer.finalize_checksum())
                            } else {
                                None
                            }
                        })
                    };
                    if let Some(sha256_checksum) = final_checksum {
                        let _ = bg_outbox
                            .send(AppMessage::FileTransferComplete {
                                transfer_id: bg_transfer_id,
                                sha256_checksum,
                            })
                            .await;
                    }
                });
            }
        }

        let _ = self
            .shared
            .event_tx
            .send(EngineEvent::FileTransferResumed { transfer_id })
            .await;
        Ok(())
    }

    /// Trigger a fresh mDNS browse query and restart the advertisement.
    /// Called by the Mac "Scan" button — surfaces peers that came online
    /// since the last browse.
    pub async fn rescan_peers(&self) {
        if let Some(tx) = &self.shared.discovery_tx {
            let state = self.shared.network_state.lock().await;
            let bind_ip = state
                .active_interface
                .as_ref()
                .map(|i| i.ip)
                .unwrap_or(state.bind_addr.ip());
            let _ = tx
                .send(DiscoveryCommand::Restart {
                    bind_ip,
                    port: self.shared.config.port,
                })
                .await;
        }
    }

    /// Re-push a received clipboard item (by content hash) to connected peers.
    /// Used when the user taps "Send" on a feed row on the Mac.
    pub async fn repush_clipboard_hash(&self, hash: String, target: SyncTarget) -> Result<()> {
        // Look up the text from the clipboard store by hash.
        let text = self
            .shared
            .clipboard_store
            .lock()
            .await
            .get_text_by_hash(&hash)
            .context("clipboard item not found by hash")?;
        self.push_clipboard_to(ClipboardContent::Text(text), target)
            .await;
        Ok(())
    }

    /// Returns this engine's stable device UUID.
    /// Used by the Android JNI bridge to filter out self-connections during NSD.
    pub fn device_id(&self) -> Uuid {
        self.shared.config.device_id
    }

    pub fn local_device_id(&self) -> Uuid {
        self.shared.config.device_id
    }

    pub fn local_device_name(&self) -> String {
        self.shared.config.device_name.clone()
    }

    /// Returns this device's Noise public-key fingerprint as a lowercase hex string.
    /// Displayed in the Mac Security pane and Android pairing screen for manual verification.
    pub fn local_fingerprint(&self) -> String {
        self.shared
            .identity_key
            .read()
            .unwrap()
            .public_bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(":")
    }

    /// Atomically update sync-filter flags from a live settings change.
    /// The router checks `settings` on every clipboard event — no restart needed.
    pub async fn apply_sync_settings(
        &self,
        sync_enabled: bool,
        sync_text: bool,
        sync_images: bool,
        sync_files: bool,
    ) {
        let mut settings = self.shared.settings.lock().await;
        settings.sync_enabled = sync_enabled;
        settings.sync_text = sync_text;
        settings.sync_images = sync_images;
        settings.sync_files = sync_files;
        tracing::info!(
            sync_enabled,
            sync_text,
            sync_images,
            sync_files,
            "sync settings updated live"
        );
    }

    /// Called by the Android JNI layer when the OS reports network restored
    /// (e.g., Wi-Fi comes back after Doze). Immediately attempts to reconnect
    /// to all known trusted peers.
    pub async fn reconnect_all_peers(&self) {
        let now_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        self.shared
            .local_last_wake
            .store(now_millis, std::sync::atomic::Ordering::Relaxed);

        reconnect_known_peers(self.shared.clone()).await;
    }

    /// Broadcast sleep state to all connected peers.
    pub async fn notify_sleep_state(&self, is_asleep: bool) {
        if !is_asleep {
            let now_millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            self.shared
                .local_last_wake
                .store(now_millis, std::sync::atomic::Ordering::Relaxed);
        }
        let msg = AppMessage::DeviceSleepState { is_asleep };
        let peers = self.shared.peer_manager.all_trusted_senders();
        for (peer_id, tx) in peers {
            if self.is_trusted(peer_id).await {
                let _ = tx.send(msg.clone()).await;
            }
        }
    }

    pub async fn connect_to_peer(&self, ip: String, port: u16) -> Result<()> {
        let addr = SocketAddr::new(ip.parse().context("invalid peer IP")?, port);
        self.shared.peer_manager.note_manual_target(addr);
        match connect_once(
            self.shared.clone(),
            vec![addr],
            None,
            DiscoverySource::Manual,
            true, // Manual connection clears blocks
        )
        .await
        {
            Ok(()) => {
                self.shared.peer_manager.clear_manual_target(addr);
                Ok(())
            }
            Err(err) => {
                self.shared.peer_manager.record_manual_failure(addr);
                Err(err)
            }
        }
    }

    pub async fn reconnect_peer_by_id(&self, device_id: Uuid) -> Result<()> {
        let _ = self
            .shared
            .peer_manager
            .set_explicit_disconnect(device_id, false);

        let peer = self
            .shared
            .peer_manager
            .get(device_id)
            .context("peer not found")?;

        let endpoints = peer.socket_addrs();
        if endpoints.is_empty() {
            tracing::warn!("reconnect_peer_by_id: no endpoints known yet for {}, cleared explicit_disconnect for auto-discovery", device_id);
            return Ok(());
        }

        let shared = self.shared.clone();
        tokio::spawn(async move {
            tracing::debug!(
                peer_id = %device_id,
                endpoints = ?endpoints,
                "manual reconnect_peer_by_id triggered"
            );
            let _ = shared
                .peer_manager
                .mark_connecting(device_id, Some(endpoints[0]));
            if let Err(e) = connect_once(
                shared.clone(),
                endpoints,
                Some(device_id),
                DiscoverySource::Manual,
                true,
            )
            .await
            {
                tracing::warn!("Manual reconnect failed: {}", e);
                let _ = shared
                    .peer_manager
                    .mark_disconnected(device_id, Some(e.to_string()));
                let _ = shared
                    .event_tx
                    .send(EngineEvent::PeerDisconnected {
                        device_id,
                        device_name: None,
                        reason: Some(e.to_string()),
                    })
                    .await;
            }
        });

        Ok(())
    }

    pub async fn disconnect_peer(&self, device_id: Uuid) -> Result<bool> {
        let _ = self
            .shared
            .peer_manager
            .set_explicit_disconnect(device_id, true)?;

        let session = self.shared.peer_manager.shutdown_peer_session(device_id)?;
        if let Some(session) = session {
            if let Some(shutdown_tx) = session.shutdown_tx {
                let _ = shutdown_tx.send(SessionShutdown {
                    reason: "manually disconnected".to_string(),
                    send_bye: true,
                    explicit_disconnect: true,
                });
            }
            let _ = self
                .shared
                .event_tx
                .send(EngineEvent::PeerDisconnected {
                    device_id,
                    device_name: self
                        .shared
                        .peer_manager
                        .get(device_id)
                        .map(|peer| Some(peer.friendly_name))
                        .unwrap_or(None),
                    reason: Some("manually disconnected".into()),
                })
                .await;
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn approve_device(
        &self,
        device_id: Uuid,
        device_name: String,
        pubkey_bytes: Vec<u8>,
    ) -> Result<()> {
        let public_key: [u8; 32] = pubkey_bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("approve_device expects a 32-byte public key"))?;
        let mut trust = self.shared.trust.lock().await;
        trust.observe_peer(device_id, device_name, &public_key)?;
        trust.trust_peer(device_id)?;
        drop(trust);
        self.shared.peer_manager.update_trust(device_id, true)?;
        Ok(())
    }

    pub async fn reject_device(&self, device_id: Uuid) -> Result<()> {
        self.reject_peer(device_id).await
    }

    pub async fn trusted_devices(&self) -> Vec<TrustRecord> {
        self.shared
            .trust
            .lock()
            .await
            .all_devices()
            .cloned()
            .collect()
    }

    pub async fn generate_qr_token(&self) -> String {
        use rand::RngCore;
        let mut bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut bytes);
        let token = hex::encode(bytes);
        *self.shared.qr_auth_token.lock().await = Some(token.clone());
        token
    }

    pub async fn send_message(&self, target_device: Uuid, msg: AppMessage) -> Result<()> {
        let peers = self.shared.peer_manager.all_connected_senders();
        if let Some(tx) = peers
            .into_iter()
            .find(|(id, _)| *id == target_device)
            .map(|(_, tx)| tx)
        {
            let _ = tx.send(msg).await;
            Ok(())
        } else {
            anyhow::bail!("Peer not connected")
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn send_remote_files_query(
        &self,
        target_device: Uuid,
        request_id: Uuid,
        summary_only: bool,
        category: Option<crate::protocol::RemoteFileCategory>,
        source: Option<crate::protocol::RemoteFileSource>,
        search_query: Option<String>,
        offset: u32,
        limit: u32,
    ) {
        let msg = AppMessage::RemoteFilesQuery {
            request_id,
            origin_device: self.shared.config.device_id,
            summary_only,
            category,
            source,
            search_query,
            offset,
            limit,
        };
        let peers = self.shared.peer_manager.all_connected_senders();
        if let Some(tx) = peers
            .into_iter()
            .find(|(id, _)| *id == target_device)
            .map(|(_, tx)| tx)
        {
            let _ = tx.send(msg).await;
        }
    }

    pub async fn send_remote_files_response(
        &self,
        target_device: Uuid,
        request_id: Uuid,
        summary: Option<crate::protocol::RemoteFilesSummary>,
        files: Vec<crate::protocol::RemoteFileEntry>,
        total_matching: u32,
        error: Option<String>,
    ) {
        let msg = AppMessage::RemoteFilesResponse {
            request_id,
            summary,
            files,
            total_matching,
            error,
        };
        let peers = self.shared.peer_manager.all_connected_senders();
        if let Some(tx) = peers
            .into_iter()
            .find(|(id, _)| *id == target_device)
            .map(|(_, tx)| tx)
        {
            let _ = tx.send(msg).await;
        }
    }

    pub async fn send_remote_thumbnail_request(
        &self,
        target_device: Uuid,
        request_id: Uuid,
        file_id: u64,
        size_px: u32,
    ) {
        let msg = AppMessage::RemoteThumbnailRequest {
            request_id,
            origin_device: self.shared.config.device_id,
            file_id,
            size_px,
        };
        let peers = self.shared.peer_manager.all_connected_senders();
        if let Some(tx) = peers
            .into_iter()
            .find(|(id, _)| *id == target_device)
            .map(|(_, tx)| tx)
        {
            let _ = tx.send(msg).await;
        }
    }

    pub async fn send_remote_thumbnail_response(
        &self,
        target_device: Uuid,
        request_id: Uuid,
        file_id: u64,
        data: Vec<u8>,
        error: Option<String>,
    ) {
        let msg = AppMessage::RemoteThumbnailResponse {
            request_id,
            file_id,
            data,
            error,
        };
        let peers = self.shared.peer_manager.all_connected_senders();
        if let Some(tx) = peers
            .into_iter()
            .find(|(id, _)| *id == target_device)
            .map(|(_, tx)| tx)
        {
            let _ = tx.send(msg).await;
        }
    }

    pub async fn send_remote_file_pull_request(
        &self,
        target_device: Uuid,
        request_id: Uuid,
        file_id: u64,
    ) {
        let msg = AppMessage::RemoteFilePullRequest {
            request_id,
            origin_device: self.shared.config.device_id,
            file_id,
        };
        let peers = self.shared.peer_manager.all_connected_senders();
        if let Some(tx) = peers
            .into_iter()
            .find(|(id, _)| *id == target_device)
            .map(|(_, tx)| tx)
        {
            let _ = tx.send(msg).await;
        }
    }

    pub async fn send_remote_file_action_request(
        &self,
        target_device: Uuid,
        action: String,
        file_id: u64,
        new_name: Option<String>,
    ) {
        let msg = AppMessage::RemoteFileActionRequest {
            action,
            file_id,
            new_name,
        };
        let peers = self.shared.peer_manager.all_connected_senders();
        if let Some(tx) = peers
            .into_iter()
            .find(|(id, _)| *id == target_device)
            .map(|(_, tx)| tx)
        {
            let _ = tx.send(msg).await;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn query_remote_files_sync(
        &self,
        target_device: Uuid,
        summary_only: bool,
        category: Option<crate::protocol::RemoteFileCategory>,
        source: Option<crate::protocol::RemoteFileSource>,
        search_query: Option<String>,
        offset: u32,
        limit: u32,
        timeout_secs: u64,
    ) -> Result<RemoteFilesResult> {
        let request_id = Uuid::new_v4();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.shared
            .remote_file_waiters
            .lock()
            .await
            .insert(request_id, tx);
        self.send_remote_files_query(
            target_device,
            request_id,
            summary_only,
            category,
            source,
            search_query,
            offset,
            limit,
        )
        .await;
        match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), rx).await {
            Ok(Ok(res)) => Ok(res),
            Ok(Err(_)) => {
                self.shared
                    .remote_file_waiters
                    .lock()
                    .await
                    .remove(&request_id);
                anyhow::bail!("Remote files query channel closed unexpectedly")
            }
            Err(_) => {
                self.shared
                    .remote_file_waiters
                    .lock()
                    .await
                    .remove(&request_id);
                anyhow::bail!("Remote files query timed out after {}s", timeout_secs)
            }
        }
    }

    pub async fn request_remote_thumbnail_sync(
        &self,
        target_device: Uuid,
        file_id: u64,
        size_px: u32,
        timeout_secs: u64,
    ) -> Result<RemoteThumbnailResult> {
        let request_id = Uuid::new_v4();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.shared
            .remote_thumb_waiters
            .lock()
            .await
            .insert(request_id, tx);
        self.send_remote_thumbnail_request(target_device, request_id, file_id, size_px)
            .await;
        match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), rx).await {
            Ok(Ok(res)) => Ok(res),
            Ok(Err(_)) => {
                self.shared
                    .remote_thumb_waiters
                    .lock()
                    .await
                    .remove(&request_id);
                anyhow::bail!("Remote thumbnail request channel closed unexpectedly")
            }
            Err(_) => {
                self.shared
                    .remote_thumb_waiters
                    .lock()
                    .await
                    .remove(&request_id);
                anyhow::bail!("Remote thumbnail request timed out after {}s", timeout_secs)
            }
        }
    }

    pub async fn send_qr_auth(&self, target_device: Uuid, token: String) {
        let msg = AppMessage::QrAuth { token };
        let peers = self.shared.peer_manager.all_connected_senders();
        if let Some(tx) = peers
            .into_iter()
            .find(|(id, _)| *id == target_device)
            .map(|(_, tx)| tx)
        {
            let _ = tx.send(msg).await;
        } else {
            if let Some(peer) = self.shared.peer_manager.get(target_device) {
                let addrs = peer.socket_addrs();
                if !addrs.is_empty() {
                    let shared = self.shared.clone();
                    tokio::spawn(async move {
                        if let Ok(()) = connect_loop(
                            shared.clone(),
                            addrs,
                            Some(target_device),
                            DiscoverySource::Manual,
                        )
                        .await
                        {
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                            let peers = shared.peer_manager.all_connected_senders();
                            if let Some(tx) = peers
                                .into_iter()
                                .find(|(id, _)| *id == target_device)
                                .map(|(_, tx)| tx)
                            {
                                let _ = tx.send(msg).await;
                            }
                        }
                    });
                }
            }
        }
    }

    pub async fn revoke_device(&self, device_id: Uuid) -> Result<bool> {
        self.revoke_peer(device_id).await
    }

    pub async fn rename_trusted_device(
        &self,
        device_id: Uuid,
        display_name: String,
    ) -> Result<bool> {
        let renamed = {
            let mut trust = self.shared.trust.lock().await;
            trust.rename_peer(device_id, display_name)?
        };
        Ok(renamed.is_some())
    }

    pub async fn is_trusted(&self, device_id: Uuid) -> bool {
        self.shared.trust.lock().await.is_trusted(device_id)
    }

    pub async fn trust_peer(&self, device_id: Uuid) -> Result<()> {
        let changed = {
            let mut trust = self.shared.trust.lock().await;
            trust.trust_peer(device_id)?
        };
        if changed.is_some() {
            self.shared.peer_manager.update_trust(device_id, true)?;
            let _ = self.shared.peer_manager.set_auto_connect(device_id, true);

            // Push local battery and network status to the newly trusted peer.
            let mut target_tx: Option<tokio::sync::mpsc::Sender<crate::protocol::AppMessage>> =
                None;
            for (id, tx) in self.shared.peer_manager.active_senders() {
                if id == device_id {
                    target_tx = Some(tx);
                    break;
                }
            }
            if let Some(tx) = target_tx {
                let sh = self.shared.clone();
                tokio::spawn(async move {
                    if let Some((level, charging)) = *sh.local_battery.lock().await {
                        let _ = tx
                            .send(AppMessage::BatteryStatus {
                                level,
                                charging,
                                origin_device: sh.config.device_id,
                                origin_device_name: sh.config.device_name.clone(),
                            })
                            .await;
                    }
                    if let Some(net) = sh.local_network.lock().await.clone() {
                        let _ = tx
                            .send(AppMessage::NetworkStatus {
                                network_type: net,
                                origin_device: sh.config.device_id,
                                origin_device_name: sh.config.device_name.clone(),
                            })
                            .await;
                    }
                });
            }
        }
        Ok(())
    }

    pub async fn reject_peer(&self, device_id: Uuid) -> Result<()> {
        let changed = {
            let mut trust = self.shared.trust.lock().await;
            trust.reject_peer(device_id)?
        };
        if changed.is_some() {
            self.shared.peer_manager.update_trust(device_id, false)?;
            let _ = self.disconnect_peer(device_id).await;
        }
        Ok(())
    }

    pub async fn revoke_peer(&self, device_id: Uuid) -> Result<bool> {
        let removed = self.shared.trust.lock().await.revoke_peer(device_id)?;
        if removed {
            self.shared.peer_manager.update_trust(device_id, false)?;
            self.shared
                .peer_manager
                .mark_disconnected(device_id, Some("trust revoked".to_string()))?;
        }
        Ok(removed)
    }

    pub async fn unreject_peer(&self, device_id: Uuid) -> Result<bool> {
        let changed = {
            let mut trust = self.shared.trust.lock().await;
            trust.unreject_peer(device_id)?
        };
        Ok(changed)
    }

    pub async fn send_pairing_request(&self, target_device: Uuid) {
        // Clear any previous Rejected or Revoked state so the outbound connection isn't blocked.
        let _ = self.unreject_peer(target_device).await;

        // Mark that WE initiated a pairing request so the PairingResponse
        // handler accepts the response (CRIT-03 anti-spoof check).
        let _ = self
            .shared
            .peer_manager
            .set_outgoing_pairing_waiting(target_device, true);

        let pin_opt = self
            .shared
            .peer_manager
            .get(target_device)
            .and_then(|p| p.pairing_pin.clone());

        let msg = AppMessage::PairingRequest {
            origin_device: self.shared.config.device_id,
            origin_device_name: self.shared.config.device_name.clone(),
            pin: pin_opt,
        };

        let peers = self.shared.peer_manager.all_connected_senders();
        if let Some(tx) = peers
            .into_iter()
            .find(|(id, _)| *id == target_device)
            .map(|(_, tx)| tx)
        {
            let _ = tx.send(msg).await;

            let pin = self
                .shared
                .peer_manager
                .get(target_device)
                .and_then(|p| p.pairing_pin.clone())
                .unwrap_or_else(|| "------".to_string());

            let device_name = self
                .shared
                .peer_manager
                .get(target_device)
                .map(|p| p.friendly_name.clone())
                .unwrap_or_else(|| "Unknown device".to_string());

            let _ = self
                .shared
                .event_tx
                .send(EngineEvent::OutgoingPairingWaiting {
                    device_id: target_device,
                    device_name,
                    pin,
                })
                .await;
        } else {
            // Trigger a manual connection if we aren't connected yet.
            if let Some(peer) = self.shared.peer_manager.get(target_device) {
                let addrs = peer.socket_addrs();
                if !addrs.is_empty() {
                    let shared = self.shared.clone();
                    if let Ok(()) = connect_loop(
                        self.shared.clone(),
                        addrs,
                        Some(target_device),
                        DiscoverySource::Manual,
                    )
                    .await
                    {
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        let peers = self.shared.peer_manager.all_connected_senders();
                        if let Some(tx) = peers
                            .into_iter()
                            .find(|(id, _)| *id == target_device)
                            .map(|(_, tx)| tx)
                        {
                            let _ = tx.send(msg).await;

                            let pin = self.shared
                                .peer_manager
                                .get(target_device)
                                .and_then(|p| p.pairing_pin.clone())
                                .unwrap_or_else(|| "------".to_string());

                            let device_name = self.shared
                                .peer_manager
                                .get(target_device)
                                .map(|p| p.friendly_name.clone())
                                .unwrap_or_else(|| "Unknown device".to_string());

                            let _ = self.shared
                                .event_tx
                                .send(EngineEvent::OutgoingPairingWaiting {
                                    device_id: target_device,
                                    device_name,
                                    pin,
                                })
                                .await;
                        }
                    }
                }
            }
        }
    }

    pub async fn initiate_pairing(&self, target_device: Uuid) -> Result<()> {
        self.send_pairing_request(target_device).await;
        Ok(())
    }

    pub async fn report_discovered_peer(
        &self,
        device_id: Uuid,
        device_name: String,
        ip: String,
        port: u16,
    ) -> Result<()> {
        let ip_addr = ip.parse::<std::net::IpAddr>().context("invalid IP")?;
        let endpoint = std::net::SocketAddr::new(ip_addr, port);
        let _ = self.shared.peer_manager.upsert_peer(
            device_id,
            device_name,
            endpoint,
            false,
            crate::peer_manager::DiscoverySource::Manual,
        );
        Ok(())
    }
    pub async fn respond_to_pairing(&self, requester_device: Uuid, accepted: bool) -> Result<()> {
        let _ = self
            .shared
            .peer_manager
            .set_pairing_requested(requester_device, false);
        if accepted {
            // Trust them persistently
            self.trust_peer(requester_device).await?;
        }
        let msg = AppMessage::PairingResponse {
            origin_device: self.shared.config.device_id,
            accepted,
        };
        let peers = self.shared.peer_manager.all_connected_senders();
        if let Some(tx) = peers
            .into_iter()
            .find(|(id, _)| *id == requester_device)
            .map(|(_, tx)| tx)
        {
            let _ = tx.send(msg).await;
        }
        if !accepted {
            // Reject the peer in the trust store so they don't auto-reconnect
            // and re-prompt endlessly. observe_trust checks for Rejected state
            // and bails, preventing the re-prompt loop.
            // reject_peer also disconnects the session internally.
            let _ = self.reject_peer(requester_device).await;
        }
        Ok(())
    }

    /// Pause Sync: keep connection alive, suppress clipboard data flow.
    pub async fn pause_sync_peer(&self, device_id: Uuid) -> Result<bool> {
        self.shared.peer_manager.set_sync_enabled(device_id, false)
    }

    /// Resume Sync: re-enable clipboard data flow.
    pub async fn resume_sync_peer(&self, device_id: Uuid) -> Result<bool> {
        self.shared.peer_manager.set_sync_enabled(device_id, true)
    }

    /// Forget Device: remove persistent pairing and revoke trust.
    pub async fn forget_device(&self, device_id: Uuid) -> Result<bool> {
        let found = self.shared.peer_manager.forget_device(device_id)?;
        if found {
            let _ = self.shared.trust.lock().await.revoke_peer(device_id);
            // Disconnect the session — device will not auto-reconnect
            let session = self.shared.peer_manager.shutdown_peer_session(device_id)?;
            if let Some(session) = session {
                if let Some(shutdown_tx) = session.shutdown_tx {
                    let _ = shutdown_tx.send(crate::peer_manager::SessionShutdown {
                        reason: "device forgotten".to_string(),
                        send_bye: true,
                        explicit_disconnect: false,
                    });
                }
            }
        }
        Ok(found)
    }

    /// Set auto-connect for a device.
    pub async fn set_auto_connect(&self, device_id: Uuid, enabled: bool) -> Result<bool> {
        self.shared
            .peer_manager
            .set_auto_connect(device_id, enabled)
    }

    /// Returns the number of currently connected peers.
    pub fn connected_peer_count(&self) -> usize {
        self.shared.peer_manager.connected_count()
    }

    pub async fn status_snapshot(&self) -> EngineStatus {
        let state = self.shared.network_state.lock().await.clone();
        EngineStatus {
            active_interface: state.active_interface,
            bind_address: state.bind_addr,
            peers: display_peers_for_status(
                self.shared.peer_manager.list(),
                self.shared.config.device_id,
                &self.shared.config.device_name,
                state.bind_addr.ip(),
            ),
            last_sync_at: self.shared.peer_manager.last_sync_at(),
        }
    }

    pub async fn active_transfers(&self) -> Vec<serde_json::Value> {
        self.shared.file_transfers.lock().await.active_transfers()
    }

    pub async fn active_speed_tests(&self) -> Vec<serde_json::Value> {
        let tests = self.shared.speed_tests.lock().await;
        tests.iter().map(|(peer_id, s)| {
            serde_json::json!({
                "test_id": s.test_id.map(|u| u.to_string()),
                "peer_id": peer_id.to_string(),
                "phase": match s.phase {
                    crate::speed_test::SpeedTestPhase::Idle => "Idle",
                    crate::speed_test::SpeedTestPhase::Sending => "Sending",
                    crate::speed_test::SpeedTestPhase::Receiving => "Receiving",
                },
                "bytes_transferred": s.bytes_transferred.load(std::sync::atomic::Ordering::Relaxed),
                "duration_secs": s.duration_secs,
            })
        }).collect()
    }

    pub async fn camera_frames(
        &self,
    ) -> tokio::sync::MutexGuard<'_, std::collections::HashMap<Uuid, Vec<u8>>> {
        self.shared.camera_frames.lock().await
    }

    /// Spawn a background task that periodically prunes transient, untrusted
    /// peer records to prevent unbounded memory/disk growth (MED-05).
    fn spawn_peer_pruner(&self) {
        let peer_manager = self.shared.peer_manager.clone();
        let trust = self.shared.trust.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
            interval.tick().await; // skip the first immediate tick
            loop {
                interval.tick().await;
                peer_manager.prune_stale_peers();

                // Also prune stale trust records (Untrusted/Rejected not seen in 7 days)
                const TRUST_MAX_AGE: u64 = 7 * 24 * 3600;
                match trust.lock().await.prune_stale(TRUST_MAX_AGE) {
                    Ok(n) if n > 0 => {
                        tracing::info!(
                            pruned = n,
                            "pruned stale trust records (untrusted, >7 days old)"
                        );
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "trust store pruning failed");
                    }
                    _ => {}
                }
            }
        });
    }

    fn spawn_sensitive_history_pruner(&self) {
        let history = self.shared.history.clone();
        let settings = self.shared.settings.clone();
        // Only prune the cache directory, leaving user's downloaded files safely intact.
        let cache_dir = self.shared.config.data_dir.join("cache");
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut disk_prune_counter = 0;
            loop {
                interval.tick().await;
                let mut hist = history.lock().await;
                if let Err(err) = hist.purge_expired_sensitive_entries() {
                    tracing::warn!(error = %err, "sensitive history pruning failed");
                }
                
                disk_prune_counter += 1;
                // Run the expensive filesystem scan only once every 10 minutes (120 ticks of 5s)
                if disk_prune_counter >= 120 {
                    disk_prune_counter = 0;
                    let retention_days = settings.lock().await.history_retention_days;
                    if retention_days > 0 {
                        if let Err(err) = hist.purge_expired_retention(retention_days) {
                            tracing::warn!(error = %err, "retention history pruning failed");
                        }
                        
                        let cache_dir_clone = cache_dir.clone();
                        tokio::task::spawn_blocking(move || {
                            let cutoff = std::time::SystemTime::now()
                                .checked_sub(Duration::from_secs(retention_days * 86400));
                                
                            if let Some(cutoff_time) = cutoff {
                                if let Ok(entries) = std::fs::read_dir(&cache_dir_clone) {
                                    for entry in entries.flatten() {
                                        if let Ok(meta) = entry.metadata() {
                                            if meta.is_file() {
                                                if let Ok(modified) = meta.modified() {
                                                    if modified < cutoff_time {
                                                        if let Err(e) = std::fs::remove_file(entry.path()) {
                                                            tracing::warn!("Failed to prune old file {:?}: {}", entry.path(), e);
                                                        } else {
                                                            tracing::info!("Pruned old hoarded file {:?}", entry.path());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        });
                    }
                }
            }
        });
    }

    /// Background watchdog to aggressively reconnect to known endpoints for trusted
    /// peers that drop offline. Bypasses the need for mDNS discovery to trigger a reconnect.
    fn spawn_auto_reconnector(&self) {
        let shared = self.shared.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(3));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // Track when we last attempted reconnection per peer to avoid flooding.
            let mut last_attempt: std::collections::HashMap<uuid::Uuid, tokio::time::Instant> =
                std::collections::HashMap::new();
            loop {
                interval.tick().await;
                let peers = shared.peer_manager.list();
                for peer in peers {
                    // Only consider peers that are not currently connected.
                    let is_offline = peer.status
                        == crate::peer_manager::PeerConnectionState::Disconnected
                        || peer.status == crate::peer_manager::PeerConnectionState::Failed;
                    if !is_offline {
                        continue;
                    }
                    // Must be trusted + remembered + auto_connect.
                    if !peer.trusted || !peer.remembered || !peer.auto_connect {
                        continue;
                    }
                    // If explicit_disconnect was set by user, respect it.
                    if peer.explicit_disconnect {
                        continue;
                    }
                    // Rate-limit: don't attempt more than once every 15 seconds per peer.
                    let now = tokio::time::Instant::now();
                    if let Some(&last) = last_attempt.get(&peer.id) {
                        if now.duration_since(last) < Duration::from_secs(5) {
                            continue;
                        }
                    }
                    let endpoints = peer.socket_addrs();
                    if !endpoints.is_empty() {
                        last_attempt.insert(peer.id, now);
                        let shared_clone = shared.clone();
                        let peer_id = peer.id;
                        let discovery = peer.discovery;
                        tokio::spawn(async move {
                            tracing::debug!(
                                peer_id = %peer_id,
                                endpoints = ?endpoints,
                                "auto-reconnector: attempting reconnection"
                            );
                            let _ = connect_once(
                                shared_clone,
                                endpoints,
                                Some(peer_id),
                                discovery,
                                false,
                            )
                            .await;
                        });
                    }
                }
            }
        });
    }

    async fn spawn_network_monitor(&self) -> Result<()> {
        let mut changes = network_manager::spawn_network_monitor(
            self.shared.config.bind_ip,
            self.shared.config.port,
            self.shared.config.network_poll_interval,
        )?;
        let shared = self.shared.clone();

        // MED-02: task panics inside tokio::spawn are silently swallowed.
        // We attach a `JoinHandle` watcher that logs the panic payload before
        // the engine continues running without its network monitor.
        let handle = tokio::spawn(async move {
            while let Some(change) = changes.recv().await {
                if let Err(err) = handle_network_change(shared.clone(), change).await {
                    warn!(error = %err, "network change handling failed");
                }
            }
        });
        tokio::spawn(async move {
            if let Err(panic) = handle.await {
                error!(error = ?panic, "network monitor task panicked — daemon may miss interface changes");
            }
        });

        Ok(())
    }

    /// Retrieve the latest camera frame for a specific peer.
    pub fn get_latest_camera_frame(&self, peer_id: uuid::Uuid) -> Option<Vec<u8>> {
        self.shared
            .camera_frames
            .blocking_lock()
            .get(&peer_id)
            .cloned()
    }
}

fn stable_device_id(public_key: [u8; 32]) -> Uuid {
    let digest = Sha256::digest(public_key);
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn spawn_listener_supervisor(shared: EngineShared, mut rx: mpsc::Receiver<ListenerCommand>) {
    tokio::spawn(async move {
        let mut listener_task: Option<tokio::task::JoinHandle<()>> = None;

        while let Some(command) = rx.recv().await {
            match command {
                ListenerCommand::Rebind(addr) => {
                    if let Some(task) = listener_task.take() {
                        task.abort();
                        let _ = task.await;
                    }

                    match bind_server_with_retry(addr).await {
                        Ok(server) => {
                            let shared_clone = shared.clone();
                            listener_task = Some(tokio::spawn(async move {
                                run_server_loop(shared_clone, server).await;
                            }));
                        }
                        Err(err) => {
                            let message = format!(
                                "listener rebind to {addr} failed after network change: {err}"
                            );
                            let _ = shared.event_tx.send(EngineEvent::Warning(message)).await;
                        }
                    }
                }
            }
        }
    });
}

fn spawn_discovery_supervisor(shared: EngineShared, mut rx: mpsc::Receiver<DiscoveryCommand>) {
    let (peer_tx, mut peer_rx) = mpsc::channel::<PeerEvent>(64);
    let peer_shared = shared.clone();

    tokio::spawn(async move {
        while let Some(event) = peer_rx.recv().await {
            match event {
                PeerEvent::Found(peer) => {
                    if let Err(err) = on_peer_found(peer_shared.clone(), peer).await {
                        warn!(error = %err, "peer discovery connect failed");
                    }
                }
                PeerEvent::Lost(device_id) => {
                    if peer_shared.peer_manager.is_connected(device_id) {
                        continue;
                    }
                    let name = peer_shared
                        .peer_manager
                        .get(device_id)
                        .map(|peer| peer.friendly_name.clone());

                    if let Some(peer) = peer_shared.peer_manager.get(device_id) {
                        if !peer.trusted && !peer.remembered {
                            let _ = peer_shared.peer_manager.forget_device(device_id);
                        } else {
                            let _ = peer_shared.peer_manager.mark_disconnected(
                                device_id,
                                Some("mDNS announcement lost".to_string()),
                            );
                        }
                    }
                    let _ = peer_shared
                        .event_tx
                        .send(EngineEvent::PeerDisconnected {
                            device_id,
                            device_name: name,
                            reason: Some("mDNS announcement lost".into()),
                        })
                        .await;
                }
            }
        }
    });

    tokio::spawn(async move {
        let mut current: Option<Discovery> = None;

        while let Some(command) = rx.recv().await {
            match command {
                DiscoveryCommand::Restart { bind_ip, port } => {
                    if let Some(discovery) = current.take() {
                        let _ = discovery.shutdown();
                    }

                    if bind_ip.is_unspecified() {
                        continue;
                    }

                    match Discovery::new(shared.config.device_id) {
                        Ok(discovery) => {
                            let advertised = discovery.advertise(
                                &shared.config.device_name,
                                port,
                                Some(bind_ip),
                            );
                            let browsed =
                                advertised.and_then(|_| discovery.browse(peer_tx.clone()));
                            match browsed {
                                Ok(()) => {
                                    current = Some(discovery);
                                }
                                Err(err) => {
                                    let message = format!(
                                        "discovery restart on {bind_ip}:{port} failed after network change: {err}"
                                    );
                                    let _ =
                                        shared.event_tx.send(EngineEvent::Warning(message)).await;
                                }
                            }
                        }
                        Err(err) => {
                            let message = format!(
                                "creating discovery daemon after network change failed: {err}"
                            );
                            let _ = shared.event_tx.send(EngineEvent::Warning(message)).await;
                        }
                    }
                }
            }
        }
    });
}

async fn run_server_loop(shared: EngineShared, server: Server) {
    loop {
        match server.accept().await {
            Ok(stream) => {
                let shared = shared.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_incoming(shared, stream).await {
                        warn!(error = %err, "incoming connection failed");
                    }
                });
            }
            Err(err) => {
                error!(error = %err, "server accept error");
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
}

async fn bind_server_with_retry(addr: SocketAddr) -> Result<Server> {
    let mut attempt = 0u32;

    loop {
        match Server::bind(addr).await {
            Ok(server) => return Ok(server),
            Err(err) if attempt < 11 => {
                attempt += 1;
                warn!(
                    addr = %addr,
                    error = %err,
                    attempt,
                    "listener bind failed during rebind, retrying"
                );
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            Err(err) => return Err(err),
        }
    }
}

async fn send_listener_rebind(shared: &EngineShared, bind_addr: SocketAddr) -> Result<()> {
    shared
        .listener_tx
        .send(ListenerCommand::Rebind(bind_addr))
        .await
        .map_err(|_| anyhow::anyhow!("listener supervisor stopped"))
}

async fn handle_network_change(shared: EngineShared, change: NetworkChangeEvent) -> Result<()> {
    let _guard = shared.network_reconcile.lock().await;
    let previous_addr = change.previous.bind_addr;
    let current_addr = change.current.bind_addr;

    {
        let mut state = shared.network_state.lock().await;
        state.bind_addr = current_addr;
        state.active_interface = change.current.active_interface.clone();
    }

    let reason = format!(
        "network changed from {} to {} ({})",
        previous_addr,
        current_addr,
        describe_change_kinds(&change)
    );

    let should_rebind = previous_addr != current_addr;
    let should_shutdown_sessions = should_rebind
        || change
            .kinds
            .contains(&crate::network_manager::NetworkChangeKind::NetworkLost);

    if should_shutdown_sessions {
        let sessions = shared.peer_manager.shutdown_all_sessions(&reason)?;
        for session in sessions {
            if let Some(shutdown_tx) = session.shutdown_tx {
                let _ = shutdown_tx.send(SessionShutdown {
                    reason: reason.clone(),
                    send_bye: false,
                    explicit_disconnect: false,
                });
            }
        }
    }

    if should_rebind {
        send_listener_rebind(&shared, current_addr).await?;
    }

    let discovery_ip = {
        let state = shared.network_state.lock().await;
        state
            .active_interface
            .as_ref()
            .map(|i| i.ip)
            .unwrap_or(current_addr.ip())
    };
    if let Some(discovery_tx) = &shared.discovery_tx {
        let _ = discovery_tx
            .send(DiscoveryCommand::Restart {
                bind_ip: discovery_ip,
                port: shared.config.port,
            })
            .await;
    }

    let _ = shared
        .event_tx
        .send(EngineEvent::Warning(reason.clone()))
        .await;

    reconnect_known_peers(shared.clone()).await;
    Ok(())
}

fn describe_change_kinds(change: &NetworkChangeEvent) -> String {
    change
        .kinds
        .iter()
        .map(|kind| match kind {
            network_manager::NetworkChangeKind::IpChanged => "ip_changed",
            network_manager::NetworkChangeKind::InterfaceChanged => "interface_changed",
            network_manager::NetworkChangeKind::NetworkLost => "network_lost",
            network_manager::NetworkChangeKind::NetworkRestored => "network_restored",
        })
        .collect::<Vec<_>>()
        .join(",")
}

async fn reconnect_known_peers(shared: EngineShared) {
    let local_ip = shared.network_state.lock().await.bind_addr.ip();
    let peers = shared.peer_manager.list();
    let mut scheduled = HashSet::new();

    for peer in peers {
        if is_obviously_local_peer(
            peer.id,
            &peer.friendly_name,
            peer.ips.first().cloned(),
            shared.config.device_id,
            &shared.config.device_name,
            Some(local_ip),
        ) {
            continue;
        }

        if !peer.should_auto_reconnect() {
            continue;
        }

        let endpoints = peer.socket_addrs();
        if !endpoints.is_empty() {
            if !should_initiate_session(&shared, peer.id, peer.discovery).await {
                continue;
            }

            // CRITICAL FIX: Do not spawn a new connection loop if the peer is already
            // connected or currently connecting. Spawning unconditionally causes massive
            // connection storms (handshakes + diffie-hellman) which overheats mobile devices.
            let is_offline = peer.status == crate::peer_manager::PeerConnectionState::Disconnected
                || peer.status == crate::peer_manager::PeerConnectionState::Failed;
            if !is_offline {
                continue;
            }

            for &endpoint in &endpoints {
                scheduled.insert(endpoint);
            }
            let shared_clone = shared.clone();
            tokio::spawn(async move {
                if let Err(err) =
                    connect_loop(shared_clone, endpoints, Some(peer.id), peer.discovery).await
                {
                    warn!(peer_id = %peer.id, error = %err, "network-change reconnect failed");
                }
            });
        }
    }

    if scheduled.is_empty() {
        if let Some(endpoint) = guessed_hotspot_gateway_endpoint(&shared).await {
            shared.peer_manager.note_manual_target(endpoint);
            let shared_clone = shared.clone();
            tokio::spawn(async move {
                if let Err(err) =
                    connect_loop(shared_clone, vec![endpoint], None, DiscoverySource::Manual).await
                {
                    warn!(
                        addr = %endpoint,
                        error = %err,
                        "android-hotspot fallback connection failed"
                    );
                }
            });
            scheduled.insert(endpoint);
        }
    }

    for endpoint in shared.peer_manager.manual_targets() {
        if scheduled.contains(&endpoint) {
            continue;
        }

        let shared_clone = shared.clone();
        tokio::spawn(async move {
            if let Err(err) =
                connect_loop(shared_clone, vec![endpoint], None, DiscoverySource::Manual).await
            {
                warn!(addr = %endpoint, error = %err, "manual reconnect after network change failed");
            }
        });
    }
}

fn display_peers_for_status(
    peers: Vec<PeerRecord>,
    local_device_id: Uuid,
    local_device_name: &str,
    local_ip: IpAddr,
) -> Vec<PeerRecord> {
    let mut deduped: HashMap<Uuid, PeerRecord> = HashMap::new();

    for peer in peers {
        if is_obviously_local_peer(
            peer.id,
            &peer.friendly_name,
            peer.ips.first().cloned(),
            local_device_id,
            local_device_name,
            Some(local_ip),
        ) {
            continue;
        }

        let key = peer.id;
        match deduped.get(&key) {
            Some(existing) if !peer_should_replace(existing, &peer) => {}
            _ => {
                deduped.insert(key, peer);
            }
        }
    }

    let mut peers: Vec<_> = deduped
        .into_values()
        .map(|mut p| {
            p.lifecycle_state = Some(p.lifecycle_state());
            p
        })
        .collect();
    peers.sort_by(|left, right| {
        peer_display_rank(left)
            .cmp(&peer_display_rank(right))
            .then_with(|| left.friendly_name.cmp(&right.friendly_name))
            .then_with(|| left.id.cmp(&right.id))
    });
    peers
}

fn is_obviously_local_peer(
    peer_id: Uuid,
    peer_name: &str,
    peer_ip: Option<IpAddr>,
    local_device_id: Uuid,
    local_device_name: &str,
    local_ip: Option<IpAddr>,
) -> bool {
    if peer_id == local_device_id {
        return true;
    }

    matches!((peer_ip, local_ip), (Some(peer_ip), Some(local_ip)) if peer_ip == local_ip
            && peer_name
                .trim()
                .eq_ignore_ascii_case(local_device_name.trim()))
}

fn peer_should_replace(current: &PeerRecord, candidate: &PeerRecord) -> bool {
    peer_display_rank(candidate) < peer_display_rank(current)
}

fn peer_display_rank(
    peer: &PeerRecord,
) -> (u8, u8, u8, std::cmp::Reverse<u64>, std::cmp::Reverse<u64>) {
    let status_rank = match peer.status {
        PeerConnectionState::Connected => 0,
        PeerConnectionState::Connecting => 1,
        PeerConnectionState::Failed => 2,
        PeerConnectionState::Disconnected => 3,
    };
    let trust_rank = if peer.trusted { 0 } else { 1 };
    let sync_rank = if peer.sync_enabled { 0 } else { 1 };
    (
        status_rank,
        trust_rank,
        sync_rank,
        std::cmp::Reverse(peer.last_seen.unwrap_or(0)),
        std::cmp::Reverse(peer.last_sync.unwrap_or(0)),
    )
}

async fn on_peer_found(shared: EngineShared, peer: PeerInfo) -> Result<()> {
    let trusted = shared.trust.lock().await.is_trusted(peer.device_id);

    for ip in peer.addrs {
        let addr = SocketAddr::new(ip, peer.port);

        let _ = shared.peer_manager.upsert_peer(
            peer.device_id,
            peer.device_name.clone(),
            addr,
            trusted,
            DiscoverySource::Mdns,
        );

        if !should_initiate_session(&shared, peer.device_id, DiscoverySource::Mdns).await {
            continue;
        }

        if shared.peer_manager.live_endpoint(peer.device_id) == Some(addr) {
            continue;
        }

        if matches!(
            shared.peer_manager.get(peer.device_id),
            Some(record)
                if record.status == PeerConnectionState::Connecting
                    && record.socket_addrs().contains(&addr)
        ) {
            continue;
        }

        let shared_clone = shared.clone();
        tokio::spawn(async move {
            if let Err(err) = connect_loop(
                shared_clone,
                vec![addr],
                Some(peer.device_id),
                DiscoverySource::Mdns,
            )
            .await
            {
                warn!(peer_id = %peer.device_id, error = %err, "discovered peer connection failed");
            }
        });
    }

    Ok(())
}

#[tracing::instrument(skip_all, fields(device_id = tracing::field::Empty))]
async fn handle_incoming(shared: EngineShared, mut stream: TcpStream) -> Result<()> {
    network::optimize_stream(&stream, "incoming engine stream");
    let shared_clone = shared.clone();
    let hs = network::handshake_responder(
        &mut stream,
        shared.config.device_id,
        shared.config.device_name.clone(),
        shared.identity_key.clone(),
        |peer_id, peer_identity| async move {
            let store = shared_clone.trust.lock().await;
            if store.is_trusted(peer_id) {
                return true;
            }
            if let Some(record) = store.get(peer_id) {
                if record.key_fingerprint == peer_identity {
                    return true;
                }
            }
            false
        },
    )
    .await?;

    tracing::Span::current().record("device_id", tracing::field::display(hs.peer_device_id));

    let _is_trusted_peer = shared.trust.lock().await.is_trusted(hs.peer_device_id);
    if hs.is_manual_reconnect {
        tracing::debug!("Manual reconnect initiated; clearing explicit disconnect");
        let _ = shared
            .peer_manager
            .set_explicit_disconnect(hs.peer_device_id, false);
    } else if shared
        .peer_manager
        .is_explicitly_disconnected(hs.peer_device_id)
    {
        anyhow::bail!(
            "ignoring inbound session from {} because it was explicitly disconnected",
            hs.peer_device_id
        );
    }

    if hs.peer_device_id == shared.config.device_id {
        anyhow::bail!("aborting inbound connect — cannot connect to self");
    }

    // Skip if this peer already has a live, connected session.
    // Without this guard, mDNS re-announcements cause the remote peer to
    // repeatedly connect, replacing the existing session each time, which
    // creates a visible connect→disconnect→reconnect flicker in the UI.
    if shared.peer_manager.is_connected(hs.peer_device_id) {
        tracing::debug!(
            peer_id = %hs.peer_device_id,
            "dropping duplicate inbound connection — peer already has an active session"
        );
        return Ok(());
    }

    let endpoint = stream.peer_addr().context("reading remote address")?;
    let trusted = observe_trust(
        &shared,
        hs.peer_device_id,
        hs.peer_device_name.clone(),
        hs.peer_identity_pubkey_bytes,
        &hs.pin,
    )
    .await?;

    shared.peer_manager.upsert_peer(
        hs.peer_device_id,
        hs.peer_device_name.clone(),
        endpoint,
        trusted,
        DiscoverySource::Mdns,
    )?;

    let _ = shared
        .peer_manager
        .set_pairing_pin(hs.peer_device_id, Some(hs.pin.display()));

    register_session(
        shared,
        stream,
        endpoint,
        hs.peer_device_id,
        hs.peer_device_name,
        hs.session,
        trusted,
        DiscoverySource::Mdns,
        Some(hs.pin.display()),
        false, // is_outbound
    )
}

async fn connect_loop(
    shared: EngineShared,
    endpoints: Vec<SocketAddr>,
    expected_device_id: Option<Uuid>,
    discovery: DiscoverySource,
) -> Result<()> {
    if endpoints.is_empty() {
        return Ok(());
    }

    if let Some(device_id) = expected_device_id {
        if !shared
            .peer_manager
            .mark_connecting(device_id, Some(endpoints[0]))?
        {
            return Ok(());
        }
    }

    let mut backoff = Backoff::new(endpoints[0].to_string());
    loop {
        match connect_once(
            shared.clone(),
            endpoints.clone(),
            expected_device_id,
            discovery,
            false,
        )
        .await
        {
            Ok(()) => {
                for ep in &endpoints {
                    shared.peer_manager.clear_manual_target(*ep);
                }
                return Ok(());
            }
            Err(err) => {
                if let Some(device_id) = expected_device_id {
                    let _ =
                        shared
                            .peer_manager
                            .mark_failed(device_id, endpoints[0], err.to_string());
                } else {
                    for ep in &endpoints {
                        shared.peer_manager.record_manual_failure(*ep);
                    }
                }

                match backoff.next() {
                    Some(delay) => {
                        warn!(error = %err, retry_in_ms = delay.as_millis(), "peer connect multi failed");
                        tokio::time::sleep(delay).await;
                    }
                    None => {
                        let message =
                            format!("connection to multiple endpoints failed after retries: {err}");
                        let _ = shared.event_tx.send(EngineEvent::Warning(message)).await;
                        if let Some(device_id) = expected_device_id {
                            let _ = shared
                                .peer_manager
                                .mark_failed_all(device_id, err.to_string());
                        }
                        return Err(err);
                    }
                }
            }
        }
    }
}

#[tracing::instrument(skip_all, fields(device_id = ?expected_device_id))]
async fn connect_once(
    shared: EngineShared,
    endpoints: Vec<SocketAddr>,
    expected_device_id: Option<Uuid>,
    discovery: DiscoverySource,
    is_manual_reconnect: bool,
) -> Result<()> {
    if !is_manual_reconnect {
        if let Some(device_id) = expected_device_id {
            if shared.peer_manager.is_connected(device_id) {
                return Ok(());
            }
            if shared.peer_manager.is_explicitly_disconnected(device_id) {
                tracing::debug!(
                    peer_id = %device_id,
                    "aborting outbound connect — peer is explicitly disconnected"
                );
                return Ok(());
            }
        }
    }
    let started = Instant::now();
    let mut tasks = tokio::task::JoinSet::new();
    let timeout_dur = shared.config.connect_timeout;
    let delay_dur = std::time::Duration::from_millis(250);

    let mut connected_stream = None;
    let mut connected_endpoint = None;
    let mut last_err = None;

    let mut ep_iter = endpoints.into_iter();

    if let Some(first_ep) = ep_iter.next() {
        tasks.spawn(async move {
            tracing::warn!("connect_once: attempting to connect to {}", first_ep);
            let res = timeout(timeout_dur, TcpStream::connect(first_ep)).await;
            (first_ep, res)
        });
    }

    loop {
        if tasks.is_empty() && ep_iter.len() == 0 {
            break;
        }

        let mut spawn_next = false;

        if ep_iter.len() > 0 {
            tokio::select! {
                res = tasks.join_next(), if !tasks.is_empty() => {
                    match res {
                        Some(Ok((ep, Ok(Ok(stream))))) => {
                            connected_stream = Some(stream);
                            connected_endpoint = Some(ep);
                            break;
                        }
                        Some(Ok((_, Err(err)))) => {
                            last_err = Some(anyhow::anyhow!("timeout: {}", err));
                            spawn_next = true;
                        }
                        Some(Ok((_, Ok(Err(err))))) => {
                            last_err = Some(anyhow::anyhow!("io error: {}", err));
                            spawn_next = true;
                        }
                        _ => { spawn_next = true; }
                    }
                }
                _ = tokio::time::sleep(delay_dur) => {
                    spawn_next = true;
                }
            }
        } else {
            match tasks.join_next().await {
                Some(Ok((ep, Ok(Ok(stream))))) => {
                    connected_stream = Some(stream);
                    connected_endpoint = Some(ep);
                    break;
                }
                Some(Ok((_, Err(err)))) => {
                    last_err = Some(anyhow::anyhow!("timeout: {}", err));
                }
                Some(Ok((_, Ok(Err(err))))) => {
                    last_err = Some(anyhow::anyhow!("io error: {}", err));
                }
                _ => {}
            }
        }

        if spawn_next {
            if let Some(next_ep) = ep_iter.next() {
                tasks.spawn(async move {
                    tracing::warn!("connect_once: attempting to connect to {}", next_ep);
                    let res = timeout(timeout_dur, TcpStream::connect(next_ep)).await;
                    (next_ep, res)
                });
            }
        }
    }

    let mut stream = match connected_stream {
        Some(s) => s,
        None => {
            let err_msg =
                last_err.unwrap_or_else(|| anyhow::anyhow!("all connection attempts failed"));
            tracing::warn!("connect_once: all connection attempts failed: {}", err_msg);
            return Err(err_msg);
        }
    };
    let endpoint = connected_endpoint.unwrap();
    network::optimize_stream(&stream, "outgoing engine stream");

    // Always send the real device name — name is not a security concern.
    let name_to_send = &shared.config.device_name;

    let hs = network::handshake_initiator(
        &mut stream,
        shared.config.device_id,
        name_to_send,
        shared.identity_key.clone(),
        is_manual_reconnect,
    )
    .await?;

    if let Some(expected) = expected_device_id {
        anyhow::ensure!(
            expected == hs.peer_device_id,
            "peer identity changed during connect: expected {}, got {}",
            expected,
            hs.peer_device_id
        );
    }

    if hs.peer_device_id == shared.config.device_id {
        anyhow::bail!("aborting outbound connect — cannot connect to self");
    }

    // If the peer already has an active session (e.g. an incoming connection
    // was accepted while we were handshaking), don't replace it.
    if shared.peer_manager.is_connected(hs.peer_device_id) {
        tracing::debug!(
            peer_id = %hs.peer_device_id,
            "aborting outbound connect — peer already has an active session"
        );
        return Ok(());
    }

    let trusted = observe_trust(
        &shared,
        hs.peer_device_id,
        hs.peer_device_name.clone(),
        hs.peer_identity_pubkey_bytes,
        &hs.pin,
    )
    .await?;

    info!(
        peer_id = %hs.peer_device_id,
        peer_name = %hs.peer_device_name,
        addr = %endpoint,
        trusted,
        connect_ms = started.elapsed().as_millis(),
        "peer connected"
    );

    shared.peer_manager.upsert_peer(
        hs.peer_device_id,
        hs.peer_device_name.clone(),
        endpoint,
        trusted,
        discovery,
    )?;

    let _ = shared
        .peer_manager
        .set_pairing_pin(hs.peer_device_id, Some(hs.pin.display()));

    register_session(
        shared,
        stream,
        endpoint,
        hs.peer_device_id,
        hs.peer_device_name,
        hs.session,
        trusted,
        discovery,
        Some(hs.pin.display()),
        true, // is_outbound
    )
}

async fn observe_trust(
    shared: &EngineShared,
    device_id: Uuid,
    device_name: String,
    identity_pubkey: [u8; 32],
    _pin: &crate::pairing::PairingPin,
) -> Result<bool> {
    let record = {
        let mut trust = shared.trust.lock().await;
        trust.observe_peer(device_id, device_name.clone(), &identity_pubkey)?
    };

    match record.state {
        TrustState::Trusted => {
            shared.peer_manager.update_trust(device_id, true)?;
            Ok(true)
        }
        TrustState::Rejected | TrustState::Revoked => {
            shared.peer_manager.update_trust(device_id, false)?;
            anyhow::bail!("peer {} is not trusted ({:?})", device_id, record.state);
        }
        TrustState::Untrusted => {
            shared.peer_manager.update_trust(device_id, false)?;

            // We NO LONGER emit PairingRequested or OutgoingPairingWaiting here.
            // Eager TCP connections should be silent.
            // Pairing prompts are now exclusively triggered by explicit
            // AppMessage::PairingRequest packets sent when the user taps 'Pair'.

            Ok(false)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn register_session(
    shared: EngineShared,
    stream: TcpStream,
    endpoint: SocketAddr,
    peer_id: Uuid,
    peer_name: String,
    session: crate::crypto::SessionKey,
    trusted: bool,
    discovery: DiscoverySource,
    session_pin: Option<String>,
    is_outbound: bool,
) -> Result<()> {
    // 64 capacity * 4 MB chunk size = ~256 MB max queued memory.
    // If the network is slower than disk I/O, this applies backpressure to the
    // file reading loop so we don't blow up Android's memory limits.
    let (outbox_tx, mut outbox_rx) = mpsc::channel::<AppMessage>(64);
    let (file_outbox_tx, mut file_outbox_rx) = mpsc::channel::<AppMessage>(128);
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<SessionShutdown>();
    match shared
        .peer_manager
        .upsert_peer(peer_id, peer_name.clone(), endpoint, trusted, discovery)
    {
        Ok(_) => {}
        Err(e) => {
            warn!("peer discovery connect failed error={:?}", e);
            return Err(e);
        }
    }
    let (session_id, replaced, rejected_new) = shared.peer_manager.replace_live_session(
        shared.config.device_id,
        peer_id,
        is_outbound,
        endpoint,
        outbox_tx.clone(),
        file_outbox_tx.clone(),
        shutdown_tx,
    )?;

    if rejected_new {
        tracing::debug!(
            "Session with {} rejected by dedup (we already have the winning session)",
            peer_id
        );
        return Ok(());
    }

    if let Some(replaced) = replaced {
        if let Some(old_shutdown) = replaced.shutdown_tx {
            let _ = old_shutdown.send(SessionShutdown {
                reason: format!("session migrated to {}", endpoint),
                send_bye: false,
                explicit_disconnect: false,
            });
        }
    }

    let _ = shared.event_tx.try_send(EngineEvent::PeerConnected {
        device_id: peer_id,
        device_name: peer_name.clone(),
        addr: endpoint,
        trusted,
    });

    // Record in activity feed.
    {
        let feed = shared.activity.clone();
        let name = peer_name.clone();
        tokio::spawn(async move {
            feed.lock().await.record_peer_connected(peer_id, name);
        });
    }

    // Push local battery and network status to the newly connected peer if trusted.
    if trusted {
        let outbox = outbox_tx.clone();
        let sh = shared.clone();
        tokio::spawn(async move {
            if let Some((level, charging)) = *sh.local_battery.lock().await {
                let _ = outbox
                    .send(AppMessage::BatteryStatus {
                        level,
                        charging,
                        origin_device: sh.config.device_id,
                        origin_device_name: sh.config.device_name.clone(),
                    })
                    .await;
            }
            if let Some(net) = sh.local_network.lock().await.clone() {
                let _ = outbox
                    .send(AppMessage::NetworkStatus {
                        network_type: net,
                        origin_device: sh.config.device_id,
                        origin_device_name: sh.config.device_name.clone(),
                    })
                    .await;
            }
            if let Some((images, videos, apps, free, total)) = *sh.local_storage.lock().await {
                let _ = outbox
                    .send(AppMessage::StorageStatus {
                        images_bytes: images,
                        videos_bytes: videos,
                        apps_bytes: apps,
                        free_bytes: free,
                        total_bytes: total,
                        origin_device: sh.config.device_id,
                        origin_device_name: sh.config.device_name.clone(),
                    })
                    .await;
            }
        });
    }

    // If this peer is reconnecting, re-announce any unfinished outbound
    // transfers so the receiver can respond with resume_from_chunk.
    {
        let pending_shared = shared.clone();
        let pending_outbox = outbox_tx.clone();
        tokio::spawn(async move {
            let pending = pending_shared
                .file_transfers
                .lock()
                .await
                .pending_outbound_announcements_for(peer_id);
            for meta in pending {
                if pending_outbox
                    .send(AppMessage::FileTransferAnnounce { meta })
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    // MED-02: wrap the session task in a JoinHandle watcher so that panics
    // are logged rather than silently swallowed by the Tokio runtime.
    let panic_peer_name = peer_name.clone();
    let session_outbox_tx = outbox_tx.clone();
    let session_handle = tokio::spawn(async move {
        let (mut sess_tx, mut sess_rx) = PeerSession {
            stream,
            session,
            peer_device_id: peer_id,
            peer_device_name: peer_name.clone(),
        }
        .split();
        let mut heartbeat = tokio::time::interval(shared.config.heartbeat_interval);

        let last_seen = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        ));
        let ping_sent_at = std::sync::Arc::new(std::sync::Mutex::new(None::<std::time::Instant>));
        let peer_sleeping = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let rx_last_seen = last_seen.clone();
        let rx_ping_sent_at = ping_sent_at.clone();
        let rx_peer_sleeping = peer_sleeping.clone();
        let rx_shared = shared.clone();
        let rx_peer_name = peer_name.clone();
        let rx_session_outbox_tx = session_outbox_tx.clone();
        let rx_peer_id = peer_id;
        let rx_session_pin = session_pin.clone();

        enum DiskTaskMsg {
            Chunk {
                transfer_id: [u8; 16],
                chunk_index: u32,
                offset: u64,
                padding: usize,
                data: bytes::Bytes,
            },
            Complete {
                transfer_id: [u8; 16],
                sha256_checksum: String,
            },
        }

        let (disk_tx, mut disk_rx) = tokio::sync::mpsc::channel::<DiskTaskMsg>(128);
        let mut last_disk_prog_emit: std::collections::HashMap<[u8; 16], std::time::Instant> = std::collections::HashMap::new();

        let dw_shared = shared.clone();
        let dw_event_tx = shared.event_tx.clone();
        let dw_outbox_tx = session_outbox_tx.clone();
        let dw_peer_id = peer_id;
        let dw_peer_name = peer_name.clone();
        tokio::spawn(async move {
            while let Some(msg) = disk_rx.recv().await {
                match msg {
                    DiskTaskMsg::Chunk { transfer_id, chunk_index, offset, padding, data } => {
                        let io_ctx = {
                            let mut mgr = dw_shared.file_transfers.lock().await;
                            if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                                t.take_io_context()
                            } else {
                                None
                            }
                        };
                        
                        if let Some((mut file, mut hasher, last_offset)) = io_ctx {
                            let data_len = data.len();
                            let res = tokio::task::spawn_blocking(move || {
                                use sha2::Digest;
                                use std::io::{Seek, SeekFrom, Write};
                                
                                if last_offset != offset {
                                    if let Err(e) = file.seek(SeekFrom::Start(offset)) {
                                        return Err(anyhow::anyhow!("seek error: {}", e));
                                    }
                                }
                                if let Err(e) = file.write_all(&data) {
                                    return Err(anyhow::anyhow!("write error: {}", e));
                                }
                                hasher.update(&data);
                                if padding > 0 {
                                    hasher.update(vec![0u8; padding]);
                                }
                                let new_offset = offset + data.len() as u64;
                                Ok::<_, anyhow::Error>((file, hasher, new_offset))
                            }).await.unwrap();
                            
                            match res {
                                Ok((file, hasher, new_offset)) => {
                                    let mut mgr = dw_shared.file_transfers.lock().await;
                                    if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                                        t.restore_io_context(file, hasher, new_offset);
                                        let prog = t.commit_chunk(chunk_index, data_len);
                                        let should_ack = t.should_ack();
                                        let file_name = t.meta.file_name.clone();
                                        drop(mgr);
                                        
                                        
                                        let now = std::time::Instant::now();
                                        let last = last_disk_prog_emit.get(&transfer_id).copied().unwrap_or_else(|| now.checked_sub(std::time::Duration::from_secs(1)).unwrap());
                                        if now.duration_since(last).as_millis() >= 100 || prog.percent == 100 {
                                            last_disk_prog_emit.insert(transfer_id, now);
                                            let _ = dw_event_tx.send(EngineEvent::FileTransferProgress {
                                                transfer_id,
                                                from_device: dw_peer_id,
                                                file_name,
                                                percent: prog.percent,
                                                bytes_received: prog.bytes_received,
                                                total_bytes: prog.total_bytes,
                                                speed_bps: prog.speed_bps,
                                                eta_secs: prog.eta_secs,
                                            }).await;
                                        }
                                        
                                        if should_ack {
                                            let _ = dw_outbox_tx.send(AppMessage::FileChunkAck {
                                                transfer_id,
                                                last_confirmed_chunk: chunk_index,
                                            }).await;
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Disk I/O error: {}", e);
                                    let mut mgr = dw_shared.file_transfers.lock().await;
                                    mgr.cancel_inbound(&transfer_id, "disk i/o error");
                                }
                            }
                        } else {
                            tracing::error!("Missing io_ctx for chunk {} of transfer {:?}", chunk_index, transfer_id);
                        }
                    }
                    DiskTaskMsg::Complete { transfer_id, sha256_checksum } => {
                        let result = {
                            let mut mgr = dw_shared.file_transfers.lock().await;
                            if let Some(transfer) = mgr.get_inbound_mut(&transfer_id) {
                                let file_name = transfer.meta.file_name.clone();
                                let file_bytes = transfer.meta.size_bytes;
                                match transfer.finalize(sha256_checksum.clone()) {
                                    Ok(dest) => Ok((dest, file_name, file_bytes)),
                                    Err(e) => Err(e.to_string()),
                                }
                            } else {
                                Err("transfer not found".into())
                            }
                        };
                        match result {
                            Ok((dest, file_name, file_bytes)) => {
                                dw_shared.file_transfers.lock().await.remove_inbound(&transfer_id);
                                let hex_tid = hex::encode(transfer_id);
                                let dest_path_str = dest.to_string_lossy().to_string();
                                dw_shared.activity.lock().await.record_file_transfer_complete(
                                    dw_peer_id,
                                    dw_peer_name.clone(),
                                    file_name.clone(),
                                    file_bytes,
                                    hex_tid,
                                    Some(dest_path_str),
                                );
                                let _ = dw_outbox_tx.send(AppMessage::FileTransferCompleteAck {
                                    transfer_id,
                                    success: true,
                                    error: None,
                                }).await;
                                let _ = dw_event_tx.send(EngineEvent::FileTransferComplete {
                                    transfer_id,
                                    from_device: dw_peer_id,
                                    from_name: dw_peer_name.clone(),
                                    file_name,
                                    dest_path: dest,
                                }).await;
                            }
                            Err(e) => {
                                {
                                    let mut mgr = dw_shared.file_transfers.lock().await;
                                    if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                                        t.status = crate::file_transfer::TransferStatus::Failed;
                                    }
                                }
                                let hex_tid = hex::encode(transfer_id);
                                dw_shared.activity.lock().await.record_file_transfer_failed(
                                    dw_peer_id,
                                    dw_peer_name.clone(),
                                    None,
                                    hex_tid,
                                    e.clone(),
                                );
                                let _ = dw_outbox_tx.send(AppMessage::FileTransferCompleteAck {
                                    transfer_id,
                                    success: false,
                                    error: Some(e.clone()),
                                }).await;
                                let _ = dw_event_tx.send(EngineEvent::FileTransferFailed {
                                    transfer_id,
                                    from_device: dw_peer_id,
                                    reason: e,
                                }).await;
                            }
                        }
                    }
                }
            }
        });

        let mut rx_disk_tx = disk_tx.clone();
        let mut rx_task = tokio::spawn(async move {
            let touch_last_seen = || {
                rx_last_seen.store(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                    std::sync::atomic::Ordering::Relaxed,
                );
            };
            let shared = rx_shared;
            let peer_name = rx_peer_name;
            let peer_id = rx_peer_id;
            loop {
                let result = sess_rx.recv().await;
                match result {
                    Ok(AppMessage::ClipboardPush {
                        seq,
                        mut content,
                        origin_device,
                        origin_device_name,
                        relay_path,
                    }) => {
                        touch_last_seen();
                        if shared
                            .peer_manager
                            .get(peer_id)
                            .map(|peer| peer.is_sync_eligible())
                            .unwrap_or(false)
                        {
                            let _ = shared.peer_manager.update_last_sync(peer_id);
                            let display_name = if origin_device_name.is_empty() {
                                peer_name.clone()
                            } else {
                                origin_device_name.clone()
                            };

                            // Run smart clipboard transformers (URL UTM parameter stripping, whitespace cleaning)
                            crate::transformer::TransformerPipeline::default_pipeline()
                                .transform(std::sync::Arc::make_mut(&mut content));

                            // --- Clipboard Security & Throttling ---
                            let payload_size = match &*content {
                                ClipboardContent::Text(t) => t.len(),
                                ClipboardContent::Image { data, .. } => data.len(),
                                ClipboardContent::File { data, .. } => data.len(),
                            };

                            // Limit sizes to prevent OOM
                            const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024;
                            const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;

                            let allowed = match &*content {
                                ClipboardContent::Text(t) => t.len() <= MAX_TEXT_BYTES,
                                ClipboardContent::Image { data, .. } => {
                                    data.len() <= MAX_IMAGE_BYTES
                                }
                                ClipboardContent::File { data, .. } => {
                                    data.len() <= MAX_IMAGE_BYTES
                                }
                            };

                            if !allowed {
                                tracing::warn!(peer_id = %peer_id, size = payload_size, "dropped oversized clipboard payload");
                                continue;
                            }

                            // Run inbound payload through the FilterChain (e.g. executable blocking, etc.)
                            let filter_chain = crate::filter::FilterChain::from_settings(
                                &*shared.settings.lock().await,
                            );
                            if let crate::filter::Verdict::Deny { reason } =
                                filter_chain.run(&content)
                            {
                                tracing::warn!(peer_id = %peer_id, reason, "inbound clipboard payload denied by filter");
                                shared
                                    .congestion_controller
                                    .on_congestion(&shared.throttle)
                                    .await;
                                continue;
                            }

                            // Apply TokenBucket throttling for large payloads
                            shared.throttle.acquire(payload_size).await;

                            // ── Timeline-first clipboard UX ───────────────
                            let hash = hash_content(&content);
                            let hash_hex = hex::encode(hash);

                            // ── Deduplicator Check ───────────────
                            let should_apply = {
                                let mut dedup = shared.dedup.lock().await;
                                dedup.should_apply(origin_device, hash)
                            };

                            if !should_apply {
                                tracing::debug!("suppressing inbound clipboard push (dedup)");
                                // It is either an echo of our own send, or a duplicate from a second peer.
                                // Acknowledge it, but skip all local UI/clipboard updates.
                                let _ = rx_session_outbox_tx
                                    .send(AppMessage::ClipboardAck { seq })
                                    .await;
                                continue;
                            }

                            let auto_apply = shared
                                .apply_policy
                                .lock()
                                .await
                                .should_auto_apply(origin_device);

                            // Record in activity feed.
                            let activity_id = {
                                if let ClipboardContent::Text(ref text) = *content {
                                    shared
                                        .clipboard_store
                                        .lock()
                                        .await
                                        .insert(hash_hex.clone(), text.clone());
                                }
                                let mut feed = shared.activity.lock().await;
                                match &*content {
                                    ClipboardContent::Text(ref text) => feed
                                        .record_remote_clipboard_text(
                                            origin_device,
                                            display_name.clone(),
                                            text,
                                            hash_hex.clone(),
                                            relay_path.clone(),
                                        ),
                                    ClipboardContent::Image { mime, data } => feed
                                        .record_remote_clipboard_image(
                                            origin_device,
                                            display_name.clone(),
                                            mime,
                                            data.len() as u64,
                                            hash_hex.clone(),
                                            relay_path.clone(),
                                        ),
                                    ClipboardContent::File { name, data } => feed
                                        .record_file_transfer_started(
                                            origin_device,
                                            display_name.clone(),
                                            name.clone(),
                                            data.len() as u64,
                                            hash_hex.clone(),
                                            false,
                                        ),
                                }
                            };

                            // If auto-applying, mark immediately applied.
                            if auto_apply {
                                let mut feed = shared.activity.lock().await;
                                feed.record_clipboard_applied(
                                    origin_device,
                                    display_name.clone(),
                                    hash_hex.clone(),
                                );
                            }

                            // Wrap content in Arc here so all downstream users — the
                            // EngineEvent and every relay-fanout hop — share one heap
                            // allocation instead of N independent clones (MED-01).
                            // (content is already Arc<ClipboardContent>)
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::ClipboardReceived {
                                    from_device: origin_device,
                                    from_name: display_name.clone(),
                                    content: content.clone(),
                                    auto_applied: auto_apply,
                                    relay_path: relay_path.clone(),
                                    activity_id,
                                })
                                .await;
                            let _ = rx_session_outbox_tx
                                .send(AppMessage::ClipboardAck { seq })
                                .await;
                            shared
                                .congestion_controller
                                .on_success(&shared.throttle)
                                .await;

                            // Persist the incoming item to history.
                            {
                                let max_bytes = shared.settings.lock().await.max_history_text_bytes;
                                let source = display_name.clone();
                                let _ = shared
                                    .history
                                    .lock()
                                    .await
                                    .push_with_options(&content, source, max_bytes);
                            }

                            // ── Mesh fanout relay ──────────────────────────
                            // If we received from a direct peer but there are other
                            // peers in the mesh, relay onwards (excluding origin + seen).
                            // Wrap content in Arc so each relay hop shares the same
                            // heap allocation instead of cloning the full payload
                            // (MED-01 — AppMessage::clone on relay hops).
                            let fanout_peers = shared.peer_manager.active_senders();
                            let mut router = shared.mesh_router.lock().await;
                            // shared_content is already Arc-wrapped above; no further
                            // full clone needed here — each fan-out is a pointer clone
                            // plus one cheap metadata-struct clone (MED-01).
                            for (fp_id, fp_tx) in fanout_peers {
                                if fp_id == peer_id {
                                    continue;
                                }
                                let Some(fp) = shared.peer_manager.get(fp_id) else {
                                    continue;
                                };
                                if !fp.is_sync_eligible() {
                                    continue;
                                }
                                if !router.should_relay_to(hash, origin_device, fp_id, &relay_path)
                                {
                                    continue;
                                }
                                let mut extended_path = relay_path.clone();
                                extended_path.push(shared.config.device_name.clone());
                                let _ = fp_tx.try_send(AppMessage::ClipboardPush {
                                    seq,
                                    content: content.clone(),
                                    origin_device,
                                    origin_device_name: display_name.clone(),
                                    relay_path: extended_path,
                                });
                            }
                        } else {
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::Warning(format!(
                                    "ignoring clipboard payload from untrusted/paused peer {}",
                                    peer_name
                                )))
                                .await;
                        }
                    }
                    Ok(AppMessage::FileTransferAnnounce { meta }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            let _ = rx_session_outbox_tx
                                .send(AppMessage::FileTransferCancel {
                                    transfer_id: meta.transfer_id,
                                    reason: "Device not trusted (Accept pairing request first)"
                                        .to_string(),
                                })
                                .await;
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::Warning(format!(
                                    "ignoring file transfer from untrusted peer {}",
                                    peer_name
                                )))
                                .await;
                            continue;
                        }
                        let transfer_id = meta.transfer_id;
                        let file_name = meta.file_name.clone();
                        let file_bytes = meta.size_bytes;
                        let mime_type = meta.mime_type.clone();

                        // Register inbound transfer.
                        let reg_result = shared
                            .file_transfers
                            .lock()
                            .await
                            .register_inbound(meta, peer_id, peer_name.clone())
                            .map(|_| ());
                        if let Err(e) = reg_result {
                            tracing::warn!(error = %e, "rejected file transfer announce");
                            let _ = rx_session_outbox_tx
                                .send(AppMessage::FileTransferCancel {
                                    transfer_id,
                                    reason: e.to_string(),
                                })
                                .await;
                            continue;
                        }

                        // Check auto-accept policy: trusted/paired devices auto-accept without requiring manual approval.
                        let is_trusted = shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false);
                        let settings = shared.settings.lock().await.clone();
                        let auto_accept = (is_trusted || settings.auto_accept_file_transfers)
                            && (settings.auto_accept_max_bytes == 0
                                || file_bytes <= settings.auto_accept_max_bytes);

                        if auto_accept {
                            let resume_from = shared
                                .file_transfers
                                .lock()
                                .await
                                .accept_inbound_or_resume(&transfer_id)
                                .unwrap_or(0);
                            let _ = rx_session_outbox_tx
                                .send(AppMessage::FileTransferAccept {
                                    transfer_id,
                                    accepted: true,
                                    resume_from_chunk: resume_from,
                                    reject_reason: None,
                                })
                                .await;
                            // Record in feed.
                            shared.activity.lock().await.record_file_transfer_started(
                                peer_id,
                                peer_name.clone(),
                                file_name.clone(),
                                file_bytes,
                                hex::encode(transfer_id),
                                false,
                            );
                        } else {
                            // Prompt the user via event.
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::FileTransferIncoming {
                                    transfer_id,
                                    from_device: peer_id,
                                    from_name: peer_name.clone(),
                                    file_name,
                                    file_bytes,
                                    mime_type,
                                })
                                .await;
                        }
                    }
                    Ok(AppMessage::FileTransferAccept {
                        transfer_id,
                        accepted,
                        resume_from_chunk,
                        reject_reason,
                    }) => {
                        touch_last_seen();
                        if !accepted {
                            shared
                                .file_transfers
                                .lock()
                                .await
                                .cancel_outbound(&transfer_id);
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::FileTransferFailed {
                                    transfer_id,
                                    from_device: peer_id,
                                    reason: reject_reason.unwrap_or_else(|| "rejected".into()),
                                })
                                .await;
                        } else {
                            {
                                let mut mgr = shared.file_transfers.lock().await;
                                if let Some(transfer) = mgr.get_outbound_mut(&transfer_id) {
                                    transfer.resume_from(resume_from_chunk);
                                }
                            }
                            let bg_outbox = shared
                                .peer_manager
                                .file_sender(peer_id)
                                .unwrap_or(session_outbox_tx.clone());
                            let bg_shared = shared.clone();
                            let bg_event_tx = shared.event_tx.clone();
                            let bg_transfer_id = transfer_id;
                            let bg_peer_id = peer_id;
                            let mut bg_last_prog_emit: std::collections::HashMap<[u8; 16], std::time::Instant> = std::collections::HashMap::new();
                            tokio::spawn(async move {
                                const BATCH_SIZE: usize = 16;
                                'outer: loop {
                                    let (next_chunk, last_acked, total_chunks): (u32, u32, u32) = {
                                        let mut mgr = bg_shared.file_transfers.lock().await;
                                        if let Some(t) = mgr.get_outbound_mut(&bg_transfer_id) {
                                            (t.next_chunk, t.last_acked_chunk.unwrap_or(0), t.total_chunks)
                                        } else {
                                            break 'outer;
                                        }
                                    };
                                    if next_chunk >= total_chunks {
                                        break 'outer;
                                    }
                                    if next_chunk > 0
                                        && next_chunk.saturating_sub(last_acked) > 512u32
                                    {
                                        tokio::time::sleep(std::time::Duration::from_millis(15))
                                            .await;
                                        continue;
                                    }
                                    let (batch, progs) = match read_outbound_chunks(
                                        bg_shared.clone(),
                                        bg_transfer_id,
                                        BATCH_SIZE,
                                    )
                                    .await
                                    {
                                        Some((batch, progs)) => (batch, progs),
                                        None => break 'outer,
                                    };

                                    
                                    if let Some((prog, fname)) = progs.last() {
                                        let now = std::time::Instant::now();
                                        let last = bg_last_prog_emit.get(&bg_transfer_id).copied().unwrap_or_else(|| now.checked_sub(std::time::Duration::from_secs(1)).unwrap());
                                        if now.duration_since(last).as_millis() >= 100 || prog.percent == 100 {
                                            bg_last_prog_emit.insert(bg_transfer_id, now);
                                            let _ = bg_event_tx
                                                .send(EngineEvent::FileTransferProgress {
                                                    transfer_id: bg_transfer_id,
                                                    from_device: bg_peer_id,
                                                    file_name: fname.clone(),
                                                    percent: prog.percent,
                                                    bytes_received: prog.bytes_received,
                                                    total_bytes: prog.total_bytes,
                                                    speed_bps: prog.speed_bps,
                                                    eta_secs: prog.eta_secs,
                                                })
                                                .await;
                                        }
                                    }

                                    if batch.is_empty() {
                                        break;
                                    }
                                    for wire_msg in batch {
                                        if bg_outbox.send(wire_msg).await.is_err() {
                                            break 'outer;
                                        }
                                    }
                                }

                                let final_checksum = {
                                    let mut mgr = bg_shared.file_transfers.lock().await;
                                    mgr.get_outbound_mut(&bg_transfer_id).and_then(|transfer| {
                                        if transfer.is_all_sent() {
                                            Some(transfer.finalize_checksum())
                                        } else {
                                            None
                                        }
                                    })
                                };
                                if let Some(sha256_checksum) = final_checksum {
                                    let _ = bg_outbox
                                        .send(AppMessage::FileTransferComplete {
                                            transfer_id: bg_transfer_id,
                                            sha256_checksum,
                                        })
                                        .await;
                                }
                            });
                        }
                    }
                    Ok(AppMessage::FileChunk {
                        transfer_id,
                        chunk_index,
                        total_chunks: _,
                        data: payload,
                        compressed,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }

                        let data = if compressed {
                            match lz4_flex::decompress_size_prepended(&payload) {
                                Ok(d) => bytes::Bytes::from(d),
                                Err(e) => {
                                    tracing::error!("Failed to decompress file chunk: {}", e);
                                    let mut mgr = shared.file_transfers.lock().await;
                                    mgr.cancel_inbound(&transfer_id, "decompression failed");
                                    continue;
                                }
                            }
                        } else {
                            payload
                        };

                        let validation = {
                            let mut mgr = shared.file_transfers.lock().await;
                            if let Some(transfer) = mgr.get_inbound_mut(&transfer_id) {
                                if transfer.from_device != peer_id {
                                    Err(anyhow::anyhow!("peer mismatch"))
                                } else {
                                    transfer.validate_chunk(chunk_index, data.len())
                                }
                            } else {
                                Err(anyhow::anyhow!("unknown transfer"))
                            }
                        };

                        match validation {
                            Ok((offset, padding, is_duplicate)) => {
                                if is_duplicate {
                                    let mut mgr = shared.file_transfers.lock().await;
                                    if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                                        let prog = t.progress_snapshot();
                                        let should_ack = t.should_ack();
                                        let file_name = t.meta.file_name.clone();
                                        let last_confirmed = t.last_confirmed_chunk;
                                        drop(mgr);
                                        
                                        let _ = shared.event_tx.send(EngineEvent::FileTransferProgress {
                                            transfer_id,
                                            from_device: peer_id,
                                            file_name,
                                            percent: prog.percent,
                                            bytes_received: prog.bytes_received,
                                            total_bytes: prog.total_bytes,
                                            speed_bps: prog.speed_bps,
                                            eta_secs: prog.eta_secs,
                                        }).await;
                                        if should_ack {
                                            let _ = rx_session_outbox_tx.send(AppMessage::FileChunkAck {
                                                transfer_id,
                                                last_confirmed_chunk: last_confirmed,
                                            }).await;
                                        }
                                    }
                                } else {
                                    let _ = disk_tx.send(DiskTaskMsg::Chunk {
                                        transfer_id,
                                        chunk_index,
                                        offset,
                                        padding,
                                        data,
                                    }).await;
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to validate chunk: {:?}", e);
                            }
                        };
                    }
                    Ok(AppMessage::FileChunkAck {
                        transfer_id,
                        last_confirmed_chunk,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        if let Some(transfer) = shared
                            .file_transfers
                            .lock()
                            .await
                            .get_outbound_mut(&transfer_id)
                        {
                            if transfer.target_device == Some(peer_id)
                                || transfer.target_device.is_none()
                            {
                                transfer.on_chunk_ack(last_confirmed_chunk);
                                let prog = transfer.progress();
                                let fname = transfer.meta.file_name.clone();
                                let event_tx = shared.event_tx.clone();
                                let tid = transfer_id;
                                tokio::spawn(async move {
                                    let _ = event_tx.send(EngineEvent::FileTransferProgress {
                                        transfer_id: tid,
                                        from_device: peer_id,
                                        file_name: fname,
                                        percent: prog.percent,
                                        bytes_received: prog.bytes_received,
                                        total_bytes: prog.total_bytes,
                                        speed_bps: prog.speed_bps,
                                        eta_secs: prog.eta_secs,
                                    }).await;
                                });
                            }
                        }
                    }
                    Ok(AppMessage::FileTransferComplete {
                        transfer_id,
                        sha256_checksum,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        // Finalize: verify SHA-256 and write to disk.
                        let _ = rx_disk_tx.send(DiskTaskMsg::Complete {
                            transfer_id,
                            sha256_checksum,
                        }).await;
                    }
                    Ok(AppMessage::FileTransferCompleteAck {
                        transfer_id,
                        success,
                        error,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }

                        let (file_name, peer_name) = {
                            let mut mgr = shared.file_transfers.lock().await;
                            let fname = mgr
                                .get_outbound_mut(&transfer_id)
                                .map(|t| t.meta.file_name.clone())
                                .unwrap_or_default();
                            mgr.remove_outbound(&transfer_id);
                            let pname = shared
                                .peer_manager
                                .get(peer_id)
                                .map(|p| p.friendly_name.clone())
                                .unwrap_or_default();
                            (fname, pname)
                        };

                        if success {
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::FileTransferComplete {
                                    transfer_id,
                                    from_device: peer_id,
                                    from_name: peer_name,
                                    file_name,
                                    dest_path: std::path::PathBuf::new(),
                                })
                                .await;
                        } else {
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::FileTransferFailed {
                                    transfer_id,
                                    from_device: peer_id,
                                    reason: error.unwrap_or_else(|| "Unknown error".to_string()),
                                })
                                .await;
                        }
                    }
                    Ok(AppMessage::FileTransferCancel {
                        transfer_id,
                        reason,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        {
                            let mut mgr = shared.file_transfers.lock().await;
                            if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                                if t.from_device == peer_id {
                                    mgr.cancel_inbound(&transfer_id, &reason);
                                }
                            }
                            if let Some(t) = mgr.get_outbound_mut(&transfer_id) {
                                if t.target_device == Some(peer_id) || t.target_device.is_none() {
                                    mgr.cancel_outbound(&transfer_id);
                                }
                            }
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::FileTransferFailed {
                                transfer_id,
                                from_device: peer_id,
                                reason,
                            })
                            .await;
                    }
                    Ok(AppMessage::FileTransferPause { transfer_id }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        {
                            let mut mgr = shared.file_transfers.lock().await;
                            if let Some(t) = mgr.get_outbound_mut(&transfer_id) {
                                if t.target_device == Some(peer_id) || t.target_device.is_none() {
                                    t.paused = true;
                                }
                            } else if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                                if t.from_device == peer_id {
                                    t.paused = true;
                                }
                            }
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::FileTransferPaused { transfer_id })
                            .await;
                    }
                    Ok(AppMessage::FileTransferResume { transfer_id }) => {
                        touch_last_seen();
                        let mut was_outbound = false;
                        {
                            let mut mgr = shared.file_transfers.lock().await;
                            if let Some(t) = mgr.get_outbound_mut(&transfer_id) {
                                if t.target_device == Some(peer_id) || t.target_device.is_none() {
                                    t.paused = false;
                                    was_outbound = true;
                                }
                            } else if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                                if t.from_device == peer_id {
                                    t.paused = false;
                                }
                            }
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::FileTransferResumed { transfer_id })
                            .await;

                        if was_outbound {
                            // Resume the background chunk loop if we are the sender
                            let bg_outbox = shared
                                .peer_manager
                                .file_sender(peer_id)
                                .unwrap_or(session_outbox_tx.clone());
                            let bg_shared = shared.clone();
                            let bg_transfer_id = transfer_id;
                            let bg_event_tx = shared.event_tx.clone();
                            let bg_peer_id = peer_id;
                            let mut bg_last_prog_emit: std::collections::HashMap<[u8; 16], std::time::Instant> = std::collections::HashMap::new();
                            tokio::spawn(async move {
                                const BATCH_SIZE: usize = 16;
                                'outer: loop {
                                    let (next_chunk, last_acked, total_chunks): (u32, u32, u32) = {
                                        let mut mgr = bg_shared.file_transfers.lock().await;
                                        if let Some(t) = mgr.get_outbound_mut(&bg_transfer_id) {
                                            (t.next_chunk, t.last_acked_chunk.unwrap_or(0), t.total_chunks)
                                        } else {
                                            break 'outer;
                                        }
                                    };
                                    if next_chunk >= total_chunks {
                                        break 'outer;
                                    }
                                    if next_chunk > 0
                                        && next_chunk.saturating_sub(last_acked) > 512u32
                                    {
                                        tokio::time::sleep(std::time::Duration::from_millis(15))
                                            .await;
                                        continue;
                                    }
                                    let (batch, progs) = match read_outbound_chunks(
                                        bg_shared.clone(),
                                        bg_transfer_id,
                                        BATCH_SIZE,
                                    )
                                    .await
                                    {
                                        Some((batch, progs)) => (batch, progs),
                                        None => break 'outer,
                                    };

                                    
                                    if let Some((prog, fname)) = progs.last() {
                                        let now = std::time::Instant::now();
                                        let last = bg_last_prog_emit.get(&bg_transfer_id).copied().unwrap_or_else(|| now.checked_sub(std::time::Duration::from_secs(1)).unwrap());
                                        if now.duration_since(last).as_millis() >= 100 || prog.percent == 100 {
                                            bg_last_prog_emit.insert(bg_transfer_id, now);
                                            let _ = bg_event_tx
                                                .send(EngineEvent::FileTransferProgress {
                                                    transfer_id: bg_transfer_id,
                                                    from_device: bg_peer_id,
                                                    file_name: fname.clone(),
                                                    percent: prog.percent,
                                                    bytes_received: prog.bytes_received,
                                                    total_bytes: prog.total_bytes,
                                                    speed_bps: prog.speed_bps,
                                                    eta_secs: prog.eta_secs,
                                                })
                                                .await;
                                        }
                                    }

                                    if batch.is_empty() {
                                        break 'outer;
                                    }

                                    // Send the batch
                                    for wire_msg in batch {
                                        if bg_outbox.send(wire_msg).await.is_err() {
                                            break 'outer;
                                        }
                                    }
                                }

                                let final_checksum = {
                                    let mut mgr = bg_shared.file_transfers.lock().await;
                                    mgr.get_outbound_mut(&bg_transfer_id).and_then(|transfer| {
                                        if transfer.is_all_sent() {
                                            Some(transfer.finalize_checksum())
                                        } else {
                                            None
                                        }
                                    })
                                };
                                if let Some(sha256_checksum) = final_checksum {
                                    let _ = bg_outbox
                                        .send(AppMessage::FileTransferComplete {
                                            transfer_id: bg_transfer_id,
                                            sha256_checksum,
                                        })
                                        .await;
                                }
                            });
                        }
                    }
                    Ok(AppMessage::SpeedTestRequest {
                        test_id,
                        duration_secs,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }

                        let mut can_accept = false;
                        {
                            let mut tests = shared.speed_tests.lock().await;
                            // Accept if we aren't already running a test with this peer
                            let entry = tests.entry(peer_id).or_insert_with(|| {
                                crate::speed_test::SpeedTestState::new(session_outbox_tx.clone())
                            });
                            if entry.phase == crate::speed_test::SpeedTestPhase::Idle {
                                entry.start_receiving(test_id, duration_secs);
                                can_accept = true;
                            }
                        }

                        let _ = session_outbox_tx
                            .send(AppMessage::SpeedTestResponse {
                                test_id,
                                accepted: can_accept,
                                reason: if can_accept {
                                    None
                                } else {
                                    Some("Busy".into())
                                },
                            })
                            .await;
                    }
                    Ok(AppMessage::SpeedTestResponse {
                        test_id,
                        accepted,
                        reason: _,
                    }) => {
                        touch_last_seen();
                        if accepted {
                            let mut tests = shared.speed_tests.lock().await;
                            if let Some(state) = tests.get_mut(&peer_id) {
                                if state.test_id == Some(test_id) {
                                    state.start_sending(test_id, state.duration_secs);
                                }
                            }
                        } else {
                            let mut tests = shared.speed_tests.lock().await;
                            if let Some(state) = tests.get_mut(&peer_id) {
                                if state.test_id == Some(test_id) {
                                    state.reset();
                                }
                            }
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::SpeedTestComplete { test_id, peer_id })
                                .await;
                        }
                    }
                    Ok(AppMessage::SpeedTestData {
                        test_id,
                        seq: _,
                        data,
                    }) => {
                        touch_last_seen();
                        let send_stats = {
                            let mut tests = shared.speed_tests.lock().await;
                            if let Some(state) = tests.get_mut(&peer_id) {
                                if state.test_id == Some(test_id)
                                    && state.phase == crate::speed_test::SpeedTestPhase::Receiving
                                {
                                    state.handle_chunk(data.len());

                                    // Should we emit stats back to sender?
                                    if let Some(last_tick) = state.last_tick_time {
                                        if last_tick.elapsed().as_millis() >= 500 {
                                            state.last_tick_time = Some(std::time::Instant::now());
                                            Some(
                                                state
                                                    .bytes_transferred
                                                    .load(std::sync::atomic::Ordering::Relaxed),
                                            )
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        };

                        if let Some(bytes) = send_stats {
                            let _ = session_outbox_tx
                                .send(AppMessage::SpeedTestStats {
                                    test_id,
                                    received_bytes: bytes,
                                })
                                .await;

                            let duration_secs = {
                                let tests = shared.speed_tests.lock().await;
                                tests.get(&peer_id).map(|s| s.duration_secs)
                            };
                            if let Some(dur) = duration_secs {
                                let _ = shared
                                    .event_tx
                                    .send(EngineEvent::SpeedTestProgress {
                                        test_id,
                                        peer_id,
                                        direction: "download".to_string(),
                                        bytes_transferred: bytes,
                                        duration_secs: dur,
                                    })
                                    .await;
                            }
                        }
                    }
                    Ok(AppMessage::SpeedTestStats {
                        test_id,
                        received_bytes,
                    }) => {
                        touch_last_seen();
                        let duration_secs = {
                            let tests = shared.speed_tests.lock().await;
                            if let Some(state) = tests.get(&peer_id) {
                                if state.test_id == Some(test_id) {
                                    Some(state.duration_secs)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        };

                        if let Some(dur) = duration_secs {
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::SpeedTestProgress {
                                    test_id,
                                    peer_id,
                                    direction: "upload".to_string(),
                                    bytes_transferred: received_bytes,
                                    duration_secs: dur,
                                })
                                .await;
                        }
                    }
                    Ok(AppMessage::SpeedTestComplete { test_id }) => {
                        touch_last_seen();
                        let mut tests = shared.speed_tests.lock().await;
                        if let Some(state) = tests.get_mut(&peer_id) {
                            if state.test_id == Some(test_id) {
                                state.reset();
                            }
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::SpeedTestComplete { test_id, peer_id })
                            .await;
                    }
                    Ok(AppMessage::HistoryMetadata { entry }) => {
                        touch_last_seen();
                        // MED-03 FIX: Only accept history metadata from trusted peers.
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let _ = shared.peer_manager.update_last_sync(peer_id);
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::HistoryMetadataReceived {
                                from_device: peer_id,
                                from_name: peer_name.clone(),
                                entry,
                            })
                            .await;
                    }
                    Ok(AppMessage::DeviceSleepState { is_asleep }) => {
                        touch_last_seen();
                        tracing::info!(peer = %peer_name, is_asleep, "received device sleep state");
                        rx_peer_sleeping.store(is_asleep, std::sync::atomic::Ordering::Relaxed);
                        // Send an instant ping if they just woke up to update their last_seen_millis
                        // and prevent their local 15s grace period from expiring before our next tick.
                        if !is_asleep {
                            let ping = probe::make_ping();
                            *rx_ping_sent_at.lock().unwrap_or_else(|e| e.into_inner()) =
                                Some(std::time::Instant::now());
                            let _ = rx_session_outbox_tx.send(ping).await;
                        }
                    }
                    Ok(AppMessage::ClipboardAck { seq }) => {
                        touch_last_seen();
                        let _ = shared.peer_manager.update_last_sync(peer_id);
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::ClipboardSynced {
                                peer_device: peer_id,
                                peer_name: peer_name.clone(),
                                seq,
                            })
                            .await;
                    }
                    Ok(AppMessage::KeyRotated { new_pubkey_bytes }) => {
                        touch_last_seen();
                        // Only accept key rotation from currently trusted peers over the established AEAD tunnel.
                        if shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            let mut trust = shared.trust.lock().await;
                            if trust.rotate_peer_key(peer_id, &new_pubkey_bytes).is_ok() {
                                tracing::info!(peer_id = %peer_id, "Successfully processed KeyRotated from peer");
                            }
                        }
                    }
                    Ok(AppMessage::Ping { timestamp_ms }) => {
                        touch_last_seen();
                        let _ = rx_session_outbox_tx
                            .send(AppMessage::Pong { timestamp_ms })
                            .await;
                    }
                    Ok(AppMessage::PairingRequest {
                        origin_device,
                        origin_device_name,
                        pin: _req_pin,
                    }) => {
                        touch_last_seen();

                        // Self-healing trust: if we already trust this peer cryptographically,
                        // and they are asking to pair again (perhaps they lost their app data),
                        // automatically accept the request so they can trust us back.
                        let is_trusted = shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false);
                        if is_trusted {
                            tracing::info!(peer_id = %peer_id, "Auto-accepting pairing request from already trusted device");
                            let _ = rx_session_outbox_tx
                                .send(AppMessage::PairingResponse {
                                    origin_device: shared.config.device_id,
                                    accepted: true,
                                })
                                .await;
                            continue;
                        }

                        let _ = shared.peer_manager.set_pairing_requested(peer_id, true);

                        // Re-emit PairingRequested with the REAL name and PIN so the UI updates
                        let pin = rx_session_pin
                            .clone()
                            .or_else(|| {
                                shared.peer_manager.get(peer_id).and_then(|p| p.pairing_pin)
                            })
                            .unwrap_or_else(|| "------".to_string());
                        let _ = shared
                            .peer_manager
                            .set_pairing_pin(peer_id, Some(pin.clone()));
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::PairingRequested {
                                device_id: origin_device,
                                device_name: origin_device_name.clone(),
                                pin,
                            })
                            .await;

                        let _ = shared
                            .event_tx
                            .send(EngineEvent::PairingRequest {
                                device_id: origin_device,
                                device_name: origin_device_name,
                            })
                            .await;
                    }
                    Ok(AppMessage::PairingResponse {
                        origin_device,
                        accepted,
                    }) => {
                        touch_last_seen();

                        // CRIT-03 FIX: Only process PairingResponse if:
                        //   1. The origin_device matches the actual session peer_id
                        //      (prevents a connected peer from spoofing trust for a different device).
                        //   2. We previously sent a PairingRequest to this peer
                        //      (tracked via the pairing_requested flag set in respond_to_pairing).
                        //   3. The peer is not already trusted (prevents re-trust of revoked peers).
                        if origin_device != peer_id {
                            tracing::warn!(
                                peer_id = %peer_id,
                                claimed_device = %origin_device,
                                "ignoring PairingResponse: origin_device does not match session peer"
                            );
                            continue;
                        }

                        // Check that we actually initiated pairing with this peer.
                        // The `pairing_requested` flag is set to true in observe_trust()
                        // when we emit PairingRequested, and cleared in respond_to_pairing().
                        // A remote peer sending an unsolicited PairingResponse is rejected.
                        let we_requested_pairing = shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.outgoing_pairing_waiting)
                            .unwrap_or(false);
                        let we_already_trust_them = shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false);

                        if !we_requested_pairing && !we_already_trust_them {
                            tracing::warn!(
                                peer_id = %peer_id,
                                "ignoring unsolicited PairingResponse — no pending pairing request and not already trusted"
                            );
                            continue;
                        }

                        // Clear the pairing_requested flag now that we've received the response.
                        let _ = shared.peer_manager.set_pairing_requested(peer_id, false);
                        let _ = shared
                            .peer_manager
                            .set_outgoing_pairing_waiting(peer_id, false);

                        if !accepted {
                            tracing::info!(peer_id = %peer_id, "peer rejected pairing request");
                            let _ = shared.peer_manager.set_pairing_pin(peer_id, None);
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::PairingResponse {
                                    device_id: origin_device,
                                    accepted,
                                })
                                .await;
                            break "peer rejected pairing request".to_string();
                        } else {
                            // ── CRITICAL: Establish mutual trust ──────────────
                            // The remote peer accepted our pairing request and
                            // already trusts us (set in respond_to_pairing).
                            // We must trust them back so the connection is fully
                            // bidirectional — otherwise the dashboard shows
                            // "not connected" and file transfers fail.
                            tracing::info!(peer_id = %peer_id, "peer accepted pairing — establishing mutual trust");
                            if !we_already_trust_them {
                                let mut trust = shared.trust.lock().await;
                                let _ = trust.trust_peer(peer_id);
                                let _ = shared.peer_manager.update_trust(peer_id, true);
                            }
                            let _ = shared.peer_manager.set_auto_connect(peer_id, true);
                            let _ = shared.peer_manager.set_pairing_pin(peer_id, None);

                            // Emit PeerConnected so the UI updates immediately.
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::PeerConnected {
                                    device_id: peer_id,
                                    device_name: peer_name.clone(),
                                    addr: endpoint,
                                    trusted: true,
                                })
                                .await;
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::PairingResponse {
                                device_id: origin_device,
                                accepted,
                            })
                            .await;
                    }
                    Ok(AppMessage::QrAuth { token }) => {
                        let valid = {
                            let mut stored = shared.qr_auth_token.lock().await;
                            if let Some(t) = stored.as_ref() {
                                if t == &token {
                                    *stored = None; // Single-use token
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        };

                        if valid {
                            tracing::info!(peer_id = %peer_id, "peer provided valid QR auth token — establishing mutual trust");
                            let _ = shared.peer_manager.set_pairing_requested(peer_id, false);
                            let _ = shared
                                .peer_manager
                                .set_outgoing_pairing_waiting(peer_id, false);

                            let mut trust = shared.trust.lock().await;
                            let _ = trust.trust_peer(peer_id);
                            let _ = shared.peer_manager.update_trust(peer_id, true);
                            let _ = shared.peer_manager.set_auto_connect(peer_id, true);
                            let _ = shared.peer_manager.set_pairing_pin(peer_id, None);

                            // Emit PeerConnected so the UI updates immediately.
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::PeerConnected {
                                    device_id: peer_id,
                                    device_name: peer_name.clone(),
                                    addr: endpoint,
                                    trusted: true,
                                })
                                .await;
                        } else {
                            tracing::warn!(peer_id = %peer_id, "peer provided invalid QR auth token");
                        }
                    }
                    Ok(AppMessage::Pong { timestamp_ms: _ }) => {
                        touch_last_seen();
                        // Feed the RTT sample into the peer's quality probe
                        // using the Instant captured at send time, which is
                        // far more accurate than round-tripping wall-clock ms
                        // over the network (HIGH-03).
                        let maybe_sent_at = rx_ping_sent_at
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .take();
                        if let Some(sent_at) = maybe_sent_at {
                            let rtt_us = probe::measure_rtt_us(sent_at);
                            let result = ProbeResult::from_samples(vec![rtt_us]);
                            let mut probes = shared.quality_probes.lock().await;
                            probes
                                .entry(peer_id)
                                .or_insert_with(|| QualityProbe::new(peer_name.as_str()))
                                .record(result);
                        }
                    }
                    Ok(AppMessage::CallStateUpdate {
                        state,
                        number,
                        contact_name,
                        origin_device,
                        origin_device_name,
                    }) => {
                        touch_last_seen();
                        // MED-03 FIX: Only process call state from trusted peers.
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        // Persist in shared state for IPC status polling.
                        {
                            let mut call = shared.active_call.lock().await;
                            if state == "idle" {
                                *call = None;
                            } else {
                                *call = Some(ActiveCallState {
                                    device_id: origin_device,
                                    device_name: origin_device_name.clone(),
                                    state: state.clone(),
                                    number: number.clone(),
                                    contact_name: contact_name.clone(),
                                });
                            }
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::CallStateChanged {
                                from_device: origin_device,
                                from_name: origin_device_name,
                                state,
                                number,
                                contact_name,
                            })
                            .await;
                    }
                    Ok(AppMessage::BatteryStatus {
                        level,
                        charging,
                        origin_device,
                        origin_device_name,
                    }) => {
                        touch_last_seen();
                        tracing::info!(
                            "Received BatteryStatus: level={}, charging={} from {}",
                            level,
                            charging,
                            origin_device
                        );
                        // MED-03 FIX: Only process battery status from trusted peers.
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            tracing::warn!(
                                "Ignoring BatteryStatus from untrusted peer {}",
                                peer_id
                            );
                            continue;
                        }
                        // Persist in shared state for IPC status polling.
                        {
                            let mut batteries = shared.peer_batteries.lock().await;
                            batteries.insert(
                                origin_device,
                                PeerBatteryState {
                                    device_id: origin_device,
                                    device_name: origin_device_name.clone(),
                                    level,
                                    charging,
                                },
                            );
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::BatteryStateChanged {
                                from_device: origin_device,
                                from_name: origin_device_name,
                                level,
                                charging,
                            })
                            .await;
                    }
                    Ok(AppMessage::NetworkStatus {
                        network_type,
                        origin_device,
                        origin_device_name,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        {
                            let mut networks = shared.peer_networks.lock().await;
                            networks.insert(
                                origin_device,
                                PeerNetworkState {
                                    device_id: origin_device,
                                    device_name: origin_device_name.clone(),
                                    network_type: network_type.clone(),
                                },
                            );
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::NetworkStateChanged {
                                from_device: origin_device,
                                from_name: origin_device_name,
                                network_type,
                            })
                            .await;
                    }
                    Ok(AppMessage::StorageStatus {
                        images_bytes,
                        videos_bytes,
                        apps_bytes,
                        free_bytes,
                        total_bytes,
                        origin_device,
                        origin_device_name,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        {
                            let mut storages = shared.peer_storage.lock().await;
                            storages.insert(
                                origin_device,
                                PeerStorageState {
                                    device_id: origin_device,
                                    device_name: origin_device_name.clone(),
                                    images_bytes,
                                    videos_bytes,
                                    apps_bytes,
                                    free_bytes,
                                    total_bytes,
                                },
                            );
                        }
                    }
                    Ok(AppMessage::CallAction {
                        action,
                        origin_device,
                    }) => {
                        touch_last_seen();
                        if action == "system:explicit_disconnect" {
                            tracing::info!("Peer explicitly disconnected. Pausing auto-reconnect.");
                            let _ = shared.peer_manager.set_explicit_disconnect(peer_id, true);
                            break "explicitly disconnected by peer".to_string();
                        }
                        tracing::info!("Received CallAction: {} from {:?}", action, origin_device);
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::CallActionRequest {
                                action,
                                from_device: origin_device,
                            })
                            .await;
                    }
                    Ok(AppMessage::Bye) => {
                        // Do NOT set explicit_disconnect here — receiving Bye from
                        // the remote peer (e.g., Android OS killed the socket) is
                        // not a user-initiated disconnect. Auto-reconnect must stay
                        // enabled so the watchdog can re-establish the link.
                        break "peer closed session".to_string();
                    }
                    Ok(AppMessage::PermissionError {
                        feature,
                        message,
                        origin_device: _,
                        origin_device_name: _,
                    }) => {
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::Warning(format!("{}: {}", feature, message)))
                            .await;
                    }
                    Ok(AppMessage::NotificationRelay {
                        id,
                        package,
                        title,
                        text,
                        origin_device,
                        origin_device_name,
                    }) => {
                        touch_last_seen();
                        // MED-03 FIX: Only process notifications from trusted peers.
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let _activity_id = {
                            let mut feed = shared.activity.lock().await;
                            feed.record_remote_notification(
                                origin_device,
                                origin_device_name.clone(),
                                package.clone(),
                                title.clone(),
                                text.clone(),
                            )
                        };
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::NotificationReceived {
                                id,
                                package,
                                title,
                                text,
                                from_device: origin_device,
                                from_name: origin_device_name,
                            })
                            .await;
                    }
                    Ok(AppMessage::CameraStreamRequest { origin_device }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::CameraStreamRequest {
                                from_device: origin_device,
                            })
                            .await;
                    }
                    Ok(AppMessage::CameraStreamAccept {
                        origin_device,
                        accepted,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::CameraStreamAccept {
                                from_device: origin_device,
                                accepted,
                            })
                            .await;
                    }
                    Ok(AppMessage::CameraStreamStop { origin_device }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        shared.camera_frames.lock().await.remove(&origin_device);
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::CameraStreamStop {
                                from_device: origin_device,
                            })
                            .await;
                    }
                    Ok(AppMessage::CameraFrame {
                        origin_device,
                        data,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        shared
                            .camera_frames
                            .lock()
                            .await
                            .insert(origin_device, data);
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::CameraFrameReceived {
                                from_device: origin_device,
                            })
                            .await;
                    }
                    Ok(AppMessage::RemoteFilesQuery {
                        request_id,
                        origin_device,
                        summary_only,
                        category,
                        source,
                        search_query,
                        offset,
                        limit,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::RemoteFilesQueryReceived {
                                request_id,
                                from_device: origin_device,
                                summary_only,
                                category,
                                source,
                                search_query,
                                offset,
                                limit,
                            })
                            .await;
                    }
                    Ok(AppMessage::RemoteFilesResponse {
                        request_id,
                        summary,
                        files,
                        total_matching,
                        error,
                    }) => {
                        touch_last_seen();
                        if let Some(tx) =
                            shared.remote_file_waiters.lock().await.remove(&request_id)
                        {
                            let _ = tx.send(RemoteFilesResult {
                                summary: summary.clone(),
                                files: files.clone(),
                                total_matching,
                                error: error.clone(),
                            });
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::RemoteFilesResponseReceived {
                                request_id,
                                from_device: peer_id,
                                summary,
                                files,
                                total_matching,
                                error,
                            })
                            .await;
                    }
                    Ok(AppMessage::RemoteThumbnailRequest {
                        request_id,
                        origin_device,
                        file_id,
                        size_px,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::RemoteThumbnailRequestReceived {
                                request_id,
                                from_device: origin_device,
                                file_id,
                                size_px,
                            })
                            .await;
                    }
                    Ok(AppMessage::RemoteThumbnailResponse {
                        request_id,
                        file_id,
                        data,
                        error,
                    }) => {
                        touch_last_seen();
                        if let Some(tx) =
                            shared.remote_thumb_waiters.lock().await.remove(&request_id)
                        {
                            let _ = tx.send(RemoteThumbnailResult {
                                file_id,
                                data: data.clone(),
                                error: error.clone(),
                            });
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::RemoteThumbnailResponseReceived {
                                request_id,
                                from_device: peer_id,
                                file_id,
                                data,
                                error,
                            })
                            .await;
                    }
                    Ok(AppMessage::RemoteFilePullRequest {
                        request_id,
                        origin_device,
                        file_id,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::RemoteFilePullRequestReceived {
                                request_id,
                                from_device: origin_device,
                                file_id,
                            })
                            .await;
                    }
                    Ok(AppMessage::RemoteFileActionRequest {
                        action,
                        file_id,
                        new_name,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::RemoteFileActionRequestReceived {
                                from_device: peer_id,
                                action,
                                file_id,
                                new_name,
                            })
                            .await;
                    }
                    Ok(AppMessage::Hello { .. }) | Ok(AppMessage::HelloAck { .. }) => {
                        // Ignored in session loop
                    }
                    Err(err) => {
                        break err.to_string();
                    }
                }
            }
        });

        let mut last_tick_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let disconnect_reason = loop {
            let now_millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let last_seen_millis = last_seen.load(std::sync::atomic::Ordering::Relaxed);

            tokio::select! {
                biased;
                shutdown = &mut shutdown_rx => {
                    match shutdown {
                        Ok(cmd) => {
                            if cmd.explicit_disconnect {
                                let _ = sess_tx.send(&AppMessage::CallAction {
                                    action: "system:explicit_disconnect".to_string(),
                                    origin_device: shared.config.device_id,
                                }).await;
                            }
                            if cmd.send_bye {
                                let _ = sess_tx.send(&AppMessage::Bye).await;
                            }
                            break cmd.reason;
                        }
                        Err(_) => {
                            break "session shutdown channel dropped".to_string();
                        }
                    }
                }
                _ = heartbeat.tick() => {
                    let tick_delta = now_millis.saturating_sub(last_tick_millis);
                    last_tick_millis = now_millis;

                    // If this tokio interval tick took significantly longer than expected (e.g. > 20s),
                    // the OS almost certainly suspended our CPU (Doze mode or sleep).
                    // We implicitly grant ourselves a fresh 15-second grace period.
                    if tick_delta > (shared.config.heartbeat_interval.as_millis() as u64) + 5000 {
                        shared.local_last_wake.store(now_millis, std::sync::atomic::Ordering::Relaxed);
                    }

                    let is_sleeping = peer_sleeping.load(std::sync::atomic::Ordering::Relaxed);
                    let timeout = if is_sleeping {
                        // 24 hours timeout if peer is sleeping
                        24 * 60 * 60 * 1000
                    } else {
                        shared.config.heartbeat_timeout.as_millis() as u64
                    };

                    let time_since_seen = now_millis.saturating_sub(last_seen_millis);
                    let time_since_wake = now_millis.saturating_sub(shared.local_last_wake.load(std::sync::atomic::Ordering::Relaxed));

                    // Only timeout if we haven't seen a heartbeat in `timeout` ms AND
                    // we've been awake for at least `timeout` ms.
                    // This prevents us from disconnecting immediately when our own CPU wakes from deep sleep.
                    if time_since_seen > timeout && time_since_wake > timeout {
                        break format!("heartbeat timeout (sleeping: {is_sleeping}, time_since_seen: {time_since_seen}, time_since_wake: {time_since_wake})");
                    }

                    shared.file_transfers.lock().await.prune_stale_transfers();

                    // Only send a ping if awake, OR if asleep and we haven't seen them for 5 minutes
                    let should_ping = if !is_sleeping {
                        true
                    } else {
                        let last_ping_elapsed = ping_sent_at.lock().unwrap_or_else(|e| e.into_inner()).map(|i| i.elapsed().as_millis() as u64).unwrap_or(u64::MAX);
                        last_ping_elapsed > 5 * 60 * 1000
                    };

                    if should_ping {
                        let ping = probe::make_ping();
                        *ping_sent_at.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::time::Instant::now());
                        if let Err(err) = sess_tx.send(&ping).await {
                            break format!("heartbeat send failed: {err}");
                        }
                    }
                }
                Some(msg) = outbox_rx.recv() => {
                    if let Err(err) = sess_tx.send(&msg).await {
                        break format!("send failed: {err}");
                    }
                }
                Some(msg) = file_outbox_rx.recv() => {
                    if let Err(err) = sess_tx.send_no_flush(&msg).await {
                        break format!("send failed: {err}");
                    }
                    for _ in 0..31 {
                        match file_outbox_rx.try_recv() {
                            Ok(next_msg) => {
                                if let Err(_err) = sess_tx.send_no_flush(&next_msg).await {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    if let Err(err) = sess_tx.flush().await {
                        break format!("flush failed: {err}");
                    }
                }
                rx_res = &mut rx_task => {
                    match rx_res {
                        Ok(reason) => break reason,
                        Err(_) => break "rx task panicked".to_string(),
                    }
                }
            }
        };

        rx_task.abort();
        let reason = Some(disconnect_reason);
        match shared
            .peer_manager
            .mark_disconnected_if_current(peer_id, session_id, reason.clone())
        {
            Ok(Some(connected_at)) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let duration = now.saturating_sub(connected_at);
                if duration < 15 {
                    let _ = shared.event_tx.send(EngineEvent::Warning(
                        format!("Device '{}' disconnected rapidly ({}s). If this is an Android device, please ensure 'Ignore Battery Optimizations' (Background Execution) is enabled in its settings.", peer_name, duration)
                    )).await;
                }

                tracing::warn!(
                    "peer disconnected: peer_id={}, reason={:?}",
                    peer_id,
                    reason
                );
                let _ = shared
                    .event_tx
                    .send(EngineEvent::PeerDisconnected {
                        device_id: peer_id,
                        device_name: Some(peer_name.clone()),
                        reason: reason.clone(),
                    })
                    .await;

                shared.dedup.lock().await.remove_peer(peer_id);

                shared
                    .file_transfers
                    .lock()
                    .await
                    .pause_all_for_device(peer_id);
                shared.camera_frames.lock().await.remove(&peer_id);

                // FIX: Phantom Pairing Prompts. Clear incoming pairing state if connection drops.
                let _ = shared.peer_manager.set_pairing_requested(peer_id, false);
                let _ = shared.peer_manager.set_pairing_pin(peer_id, None);

                // Record in activity feed.
                let feed = shared.activity.clone();
                let name = peer_name.clone();
                let disc_reason = reason.clone();
                tokio::spawn(async move {
                    feed.lock()
                        .await
                        .record_peer_disconnected(peer_id, name, disc_reason);
                });

                if shared
                    .peer_manager
                    .get(peer_id)
                    .map(|peer| peer.should_auto_reconnect())
                    .unwrap_or(false)
                {
                    // ── AirDrop-style immediate reconnect ────────────────────
                    // Instead of waiting for the 3s auto-reconnector tick,
                    // spawn an immediate reconnect attempt after a short
                    // anti-loop delay. This cuts reconnect latency from ~3s
                    // to ~500ms for trusted peers.
                    let shared_reconnect = shared.clone();
                    let peer_endpoints = shared
                        .peer_manager
                        .get(peer_id)
                        .map(|p| p.socket_addrs())
                        .unwrap_or_default();
                    let peer_discovery = shared
                        .peer_manager
                        .get(peer_id)
                        .map(|p| p.discovery)
                        .unwrap_or(DiscoverySource::Unknown);
                    if !peer_endpoints.is_empty() {
                        tokio::spawn(async move {
                            // Small delay to prevent 0-delay infinite loops
                            // if the remote immediately resets.
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            // Only attempt if still disconnected (auto-reconnector
                            // may have already picked it up).
                            let still_offline = shared_reconnect
                                .peer_manager
                                .get(peer_id)
                                .map(|p| {
                                    p.status
                                        == crate::peer_manager::PeerConnectionState::Disconnected
                                        || p.status
                                            == crate::peer_manager::PeerConnectionState::Failed
                                })
                                .unwrap_or(false);
                            if still_offline {
                                tracing::debug!(
                                    peer_id = %peer_id,
                                    "immediate reconnect: attempting fast recovery"
                                );
                                let _ = connect_once(
                                    shared_reconnect,
                                    peer_endpoints,
                                    Some(peer_id),
                                    peer_discovery,
                                    false,
                                )
                                .await;
                            }
                        });
                    }
                }
            }
            Ok(None) => {}
            Err(err) => {
                warn!(peer_id = %peer_id, error = %err, "failed to mark peer disconnected");
            }
        }
    });

    // MED-02: observe the session task handle so panics surface as log errors
    // instead of being silently discarded by the Tokio runtime.
    tokio::spawn(async move {
        if let Err(panic) = session_handle.await {
            error!(
                peer_id = %peer_id,
                peer_name = %panic_peer_name,
                error = ?panic,
                "peer session task panicked — peer will appear disconnected"
            );
        }
    });

    Ok(())
}

async fn should_initiate_session(
    shared: &EngineShared,
    peer_id: Uuid,
    discovery: DiscoverySource,
) -> bool {
    if shared.peer_manager.is_explicitly_disconnected(peer_id) {
        return false;
    }
    if shared.peer_manager.is_connected(peer_id) && discovery != DiscoverySource::Manual {
        return false;
    }
    match discovery {
        DiscoverySource::Manual => true,
        DiscoverySource::Mdns
        | DiscoverySource::Unknown
        | DiscoverySource::UdpBeacon
        | DiscoverySource::UdpMulticast
        | DiscoverySource::HotspotProbe => {
            // Both sides attempt connection eagerly. If both succeed, the
            // lower-ID peer's session wins via replace_live_session (the
            // existing dedup logic). This is critical for asymmetric routing
            // (e.g. Android hotspot AP isolation) where only one direction
            // may be routable.
            true
        }
    }
}

async fn guessed_hotspot_gateway_endpoint(shared: &EngineShared) -> Option<SocketAddr> {
    let state = shared.network_state.lock().await.clone();
    let iface = state.active_interface.as_ref()?;
    let gateway = network_manager::detect_android_hotspot_gateway(iface)?;
    Some(SocketAddr::new(gateway, shared.config.port))
}

fn resolve_bind_address(
    config: &EngineConfig,
) -> Result<(Option<NetworkInterfaceInfo>, SocketAddr)> {
    let snapshot = network_manager::resolve_snapshot(config.bind_ip, config.port)?;
    Ok((snapshot.active_interface, snapshot.bind_addr))
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {:?}", parent))?;
    }
    Ok(())
}
