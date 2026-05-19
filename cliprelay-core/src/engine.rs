use crate::activity::ActivityFeed;
use crate::dedup::hash_content;
use crate::discovery::{Discovery, PeerEvent, PeerInfo};
use crate::file_transfer::{default_save_dir, FileTransferManager, FileTransferMessage};
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
use crate::settings::{default_peer_store_path, default_trust_store_path, Settings};
use crate::trust::{format_fingerprint, TrustRecord, TrustState, TrustStore};
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::timeout;
use tracing::{error, info, warn};
use uuid::Uuid;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// RFC 7396 JSON merge-patch: recursively overwrite `target` with non-null
/// fields from `patch`, removing null-keyed fields.
fn json_merge_patch(target: &mut serde_json::Value, patch: &serde_json::Value) {
    if let serde_json::Value::Object(patch_obj) = patch {
        if !target.is_object() {
            *target = serde_json::Value::Object(serde_json::Map::new());
        }
        let target_obj = target.as_object_mut().unwrap();
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

#[derive(Debug)]
pub enum EngineEvent {
    /// A remote clipboard item arrived and was added to the activity feed.
    /// If `auto_applied` is true it was also written to the local clipboard.
    ClipboardReceived {
        from_device: Uuid,
        from_name: String,
        content: ClipboardContent,
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
    TofuPrompt {
        device_id: Uuid,
        device_name: String,
        fingerprint_display: String,
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

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            device_id: Uuid::nil(),
            device_name: whoami::devicename(),
            port: DEFAULT_PORT,
            trust_store_path: default_trust_store_path(),
            peer_store_path: default_peer_store_path(),
            identity_path: IdentityStore::default_path(),
            connect_timeout: Duration::from_secs(2),
            heartbeat_interval: Duration::from_secs(5),
            heartbeat_timeout: Duration::from_secs(15),
            bind_ip: None,
            enable_discovery: true,
            network_poll_interval: Duration::from_secs(2),
            data_dir: default_peer_store_path()
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".")),
            file_save_dir: None,
            history_limit: Some(500),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EngineStatus {
    pub active_interface: Option<NetworkInterfaceInfo>,
    pub bind_address: SocketAddr,
    pub peers: Vec<PeerRecord>,
    pub last_sync_at: Option<u64>,
}

#[derive(Debug, Clone)]
struct RuntimeNetworkState {
    bind_addr: SocketAddr,
    active_interface: Option<NetworkInterfaceInfo>,
}

#[derive(Debug)]
enum ListenerCommand {
    Rebind(SocketAddr),
}

#[derive(Debug)]
enum DiscoveryCommand {
    Restart {
        bind_ip: IpAddr,
        port: u16,
    },
    /// Force an immediate re-browse without changing bind address/port.
    /// Used by the Mac "Scan" button and Android NSD retry.
    Rescan,
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

#[derive(Clone)]
struct EngineShared {
    config: EngineConfig,
    trust: Arc<Mutex<TrustStore>>,
    peer_manager: Arc<PeerManager>,
    event_tx: mpsc::Sender<EngineEvent>,
    identity_pubkey: [u8; 32],
    network_state: Arc<Mutex<RuntimeNetworkState>>,
    listener_tx: mpsc::Sender<ListenerCommand>,
    discovery_tx: Option<mpsc::Sender<DiscoveryCommand>>,
    network_reconcile: Arc<Mutex<()>>,
    // ── New: mesh-aware shared state ─────────────────────────────────────────
    /// Mesh fanout router + relay dedup (shared, lock-protected).
    mesh_router: Arc<Mutex<MeshRouter>>,
    /// Cross-device activity feed.
    activity: Arc<Mutex<ActivityFeed>>,
    /// File transfer manager.
    file_transfers: Arc<Mutex<FileTransferManager>>,
    /// Clipboard apply policy (timeline-first vs auto-apply).
    apply_policy: Arc<Mutex<ClipboardApplyPolicy>>,
    /// Settings snapshot for policy decisions (updated lazily).
    settings: Arc<Mutex<Settings>>,
    /// Per-peer link-quality probes — drives adaptive chunk sizing (HIGH-03).\n    /// Keyed by peer device UUID; populated on first Pong receipt.
    quality_probes: Arc<Mutex<std::collections::HashMap<uuid::Uuid, QualityProbe>>>,
    /// Clipboard content store — maps content hash → text payload for repush.
    clipboard_store: Arc<Mutex<crate::engine_support::ClipboardStore>>,
    /// Local clipboard reader (platform abstraction for push_current_clipboard).
    local_clipboard: Arc<Mutex<crate::engine_support::LocalClipboard>>,
    /// Persistent history store.
    history: Arc<Mutex<crate::history::History>>,
    /// In-memory feedback event log (most-recent N events).
    feedback: Arc<Mutex<crate::engine_support::FeedbackLog>>,
    /// Active phone call state (set on ringing/offhook, cleared on idle).
    active_call: Arc<Mutex<Option<ActiveCallState>>>,
    /// Per-peer battery levels (F20). Keyed by device UUID.
    peer_batteries: Arc<Mutex<std::collections::HashMap<uuid::Uuid, PeerBatteryState>>>,
}

pub struct Engine {
    shared: EngineShared,
    seq: Arc<Mutex<u64>>,
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
            identity_pubkey: identity.public_bytes,
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
                        let tmp = std::env::temp_dir().join("cliprelay_history_fallback.json");
                        crate::history::History::load_with_limit(&tmp, limit)
                            .expect("cannot create fallback history store")
                    },
                )
            })),
            feedback: Arc::new(Mutex::new(crate::engine_support::FeedbackLog::new(200))),
            active_call: Arc::new(Mutex::new(None)),
            peer_batteries: Arc::new(Mutex::new(std::collections::HashMap::new())),
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
        if let Some(discovery_tx) = &engine.shared.discovery_tx {
            let _ = discovery_tx
                .send(DiscoveryCommand::Restart {
                    bind_ip: initial_bind.ip(),
                    port: engine.shared.config.port,
                })
                .await;
        }

        engine.spawn_network_monitor().await?;
        engine.spawn_peer_pruner();
        engine.spawn_sensitive_history_pruner();
        Ok(engine)
    }

    // ── Call Continuity ───────────────────────────────────────────────────────

    /// Push a phone call state change to all connected, trusted peers.
    /// Called by the Android JNI layer when PhoneStateListener fires.
    pub async fn push_call_state(&self, state: String, number: String, contact_name: String) {
        let msg = AppMessage::CallStateUpdate {
            state,
            number,
            contact_name,
            origin_device: self.shared.config.device_id,
            origin_device_name: self.shared.config.device_name.clone(),
        };

        let peers = self.shared.peer_manager.all_connected_senders();
        for (peer_id, tx) in peers {
            let Some(peer) = self.shared.peer_manager.get(peer_id) else { continue };
            if !peer.trusted { continue; }
            let _ = tx.send(msg.clone()).await;
        }
    }

    /// Send an accept/decline call action to a specific Android peer.
    /// Called by the macOS IPC layer when the user taps Accept or Decline.
    pub async fn send_call_action(&self, action: String, target_device: Uuid) {
        tracing::info!("send_call_action: action={}, target_device={}", action, target_device);
        let msg = AppMessage::CallAction {
            action,
            origin_device: self.shared.config.device_id,
        };

        let peers = self.shared.peer_manager.all_connected_senders();
        tracing::info!("send_call_action: all connected peers count={}", peers.len());
        for (peer_id, tx) in peers {
            tracing::info!("send_call_action: checking peer_id={}", peer_id);
            if peer_id != target_device {
                tracing::info!("send_call_action: peer_id mismatch (expected {}, got {})", target_device, peer_id);
                continue;
            }
            tracing::info!("send_call_action: peer MATCHED! Sending call action message over socket...");
            let _ = tx.send(msg.clone()).await;
        }
    }

    /// Get the current active phone call state, if any.
    /// Returns None when no call is in progress.
    pub async fn active_call(&self) -> Option<ActiveCallState> {
        self.shared.active_call.lock().await.clone()
    }

    // ── F20: Battery synchronization ──────────────────────────────────────────

    /// Push this device's battery status to all connected trusted peers.
    pub async fn push_battery_status(&self, level: u8, charging: bool) {
        let msg = AppMessage::BatteryStatus {
            level,
            charging,
            origin_device: self.shared.config.device_id,
            origin_device_name: self.shared.config.device_name.clone(),
        };

        let peers = self.shared.peer_manager.active_senders();
        for (peer_id, tx) in peers {
            let Some(peer) = self.shared.peer_manager.get(peer_id) else { continue };
            if !peer.trusted { continue; }
            let _ = tx.send(msg.clone()).await;
        }
    }

    /// Get battery states for all peers that have reported their level.
    pub async fn peer_batteries(&self) -> Vec<PeerBatteryState> {
        self.shared.peer_batteries.lock().await.values().cloned().collect()
    }

    pub async fn push_clipboard(&self, content: ClipboardContent) -> usize {
        self.push_clipboard_to(content, SyncTarget::All)
            .await
            .delivered_count()
    }

    pub async fn push_clipboard_to(
        &self,
        content: ClipboardContent,
        target: SyncTarget,
    ) -> SyncDispatchReport {
        let seq = {
            let mut guard = self.seq.lock().await;
            // Use wrapping_add so the counter rolls over safely after u64::MAX
            // instead of panicking in debug builds (LOW-01).
            *guard = guard.wrapping_add(1);
            *guard
        };

        // Hash for dedup + activity recording.
        let hash = hash_content(&content);

        // Register in mesh router so we never echo back to ourselves.
        {
            let mut router = self.shared.mesh_router.lock().await;
            router.register_local_send(hash);
        }

        // Record in activity feed.
        {
            let mut feed = self.shared.activity.lock().await;
            if let ClipboardContent::Text(ref text) = content {
                feed.record_local_clipboard_text(
                    self.shared.config.device_id,
                    self.shared.config.device_name.clone(),
                    text,
                    hex::encode(hash),
                );
            }
        }

        // Record text in clipboard_store for future repush by hash.
        if let ClipboardContent::Text(ref text) = content {
            self.shared
                .clipboard_store
                .lock()
                .await
                .insert(hex::encode(hash), text.clone());
        }

        // Optionally compress images before sending.
        let compress_enabled = self.shared.settings.lock().await.sync_images;
        let content = if matches!(content, ClipboardContent::Image { .. }) && compress_enabled {
            let (compressed, stats) = crate::compress::compress_image(content, true).await;
            if let Some(ref s) = stats {
                tracing::debug!(compression = %s, "image compressed for send");
            }
            compressed
        } else {
            content
        };

        // Re-hash after potential compression so the wire message is consistent.
        let hash = hash_content(&content);

        let relay_path = vec![self.shared.config.device_name.clone()];
        let msg = AppMessage::ClipboardPush {
            seq,
            content: content.clone(),
            origin_device: self.shared.config.device_id,
            origin_device_name: self.shared.config.device_name.clone(),
            relay_path: relay_path.clone(),
        };
        let metadata =
            HistoryMetadata::from_content(&content, self.shared.config.device_name.clone(), false);

        let peers = self.shared.peer_manager.active_senders();
        let mut report = SyncDispatchReport {
            seq,
            target: target.clone(),
            peers: Vec::new(),
        };

        for (peer_id, tx) in peers {
            let Some(peer) = self.shared.peer_manager.get(peer_id) else {
                continue;
            };

            if !peer.trusted {
                report.peers.push(SyncDispatchPeer {
                    device_id: peer_id,
                    device_name: peer.friendly_name,
                    delivered: false,
                    metadata_only: false,
                    reason: Some("peer is not trusted".into()),
                });
                continue;
            }

            if !peer.is_sync_eligible() {
                report.peers.push(SyncDispatchPeer {
                    device_id: peer_id,
                    device_name: peer.friendly_name,
                    delivered: false,
                    metadata_only: false,
                    reason: Some("sync paused for this peer".into()),
                });
                continue;
            }

            let is_target = match target {
                SyncTarget::All => true,
                SyncTarget::Device(target_id) => target_id == peer_id,
            };

            // Mesh router dedup check.
            let should_relay = {
                let mut router = self.shared.mesh_router.lock().await;
                router.should_relay_to(hash, self.shared.config.device_id, peer_id, &relay_path)
            };

            if !should_relay {
                report.peers.push(SyncDispatchPeer {
                    device_id: peer_id,
                    device_name: peer.friendly_name,
                    delivered: false,
                    metadata_only: false,
                    reason: Some("mesh dedup: already delivered".into()),
                });
                continue;
            }

            let app_message = if is_target {
                msg.clone()
            } else {
                AppMessage::HistoryMetadata {
                    entry: metadata.clone(),
                }
            };

            let send_result = match tx.try_send(app_message.clone()) {
                Ok(()) => Ok(()),
                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => tx.send(app_message).await,
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                    Err(tokio::sync::mpsc::error::SendError(app_message))
                }
            };

            match send_result {
                Ok(()) => report.peers.push(SyncDispatchPeer {
                    device_id: peer_id,
                    device_name: peer.friendly_name,
                    delivered: true,
                    metadata_only: !is_target,
                    reason: None,
                }),
                Err(_) => {
                    let reason = "peer queue unavailable".to_string();
                    let _ = self
                        .shared
                        .event_tx
                        .send(EngineEvent::ClipboardSyncFailed {
                            peer_device: peer_id,
                            peer_name: peer.friendly_name.clone(),
                            seq,
                            reason: reason.clone(),
                        })
                        .await;
                    report.peers.push(SyncDispatchPeer {
                        device_id: peer_id,
                        device_name: peer.friendly_name,
                        delivered: false,
                        metadata_only: !is_target,
                        reason: Some(reason),
                    });
                }
            }
        }

        report
    }

    // ── Activity Feed ─────────────────────────────────────────────────────────

    /// Get recent activity feed entries (up to `limit`).
    pub async fn activity_recent(&self, limit: usize) -> Vec<crate::activity::ActivityEntry> {
        self.shared
            .activity
            .lock()
            .await
            .recent(limit)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Get activity feed entries added after `since_id`.
    pub async fn activity_since(&self, since_id: u64) -> Vec<crate::activity::ActivityEntry> {
        self.shared
            .activity
            .lock()
            .await
            .since(since_id)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Get pending remote clipboard items not yet applied locally.
    pub async fn pending_remote_clipboards(&self) -> Vec<crate::activity::ActivityEntry> {
        self.shared
            .activity
            .lock()
            .await
            .pending_remote_clipboards()
            .into_iter()
            .cloned()
            .collect()
    }

    /// Explicitly apply a remote clipboard item by its content hash.
    /// Marks it applied in the feed and emits `ClipboardReceived { auto_applied: true }`.
    pub async fn apply_clipboard_by_hash(&self, content_hash: String) -> Result<bool> {
        // Find the matching pending entry.
        let entry = {
            let feed = self.shared.activity.lock().await;
            feed.pending_remote_clipboards()
                .into_iter()
                .find(|e| e.content_hash.as_deref() == Some(&content_hash))
                .cloned()
        };
        let Some(entry) = entry else {
            return Ok(false);
        };
        let from_device = entry.device_id;
        let from_name = entry.device_name.clone();
        let text = self
            .shared
            .clipboard_store
            .lock()
            .await
            .get_text_by_hash(&content_hash)
            .or(entry.text_preview.clone())
            .unwrap_or_default();
        {
            let mut feed = self.shared.activity.lock().await;
            feed.record_clipboard_applied(from_device, from_name.clone(), content_hash);
        }
        // Emit event so the platform layer writes to local clipboard.
        let _ = self
            .shared
            .event_tx
            .send(EngineEvent::ClipboardReceived {
                from_device,
                from_name,
                content: ClipboardContent::Text(text),
                auto_applied: true,
                relay_path: entry.relay_path,
                activity_id: entry.id,
            })
            .await;
        Ok(true)
    }

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

    /// Apply a JSON merge-patch to the current settings.
    pub async fn patch_settings(&self, patch: String) -> Result<()> {
        let mut current = serde_json::to_value(&*self.shared.settings.lock().await)?;
        let patch_val: serde_json::Value =
            serde_json::from_str(&patch).context("patch_settings: invalid JSON patch")?;
        json_merge_patch(&mut current, &patch_val);
        let new_settings: Settings = serde_json::from_value(current)
            .context("patch_settings: patched value is invalid Settings")?;
        self.apply_settings(new_settings).await;
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
        self.apply_settings(s).await;
        Ok(())
    }

    pub async fn set_sync_enabled(&self, enabled: bool) {
        self.shared.settings.lock().await.sync_enabled = enabled;
    }

    pub async fn set_timeline_first_mode(&self, enabled: bool) {
        self.shared
            .apply_policy
            .lock()
            .await
            .set_timeline_first(enabled);
    }

    pub async fn set_auto_apply_clipboard(&self, enabled: bool) {
        self.shared
            .apply_policy
            .lock()
            .await
            .set_auto_apply(enabled);
    }

    // ── History ───────────────────────────────────────────────────────────────

    pub async fn history_recent(&self, n: usize) -> Vec<crate::history::HistoryEntry> {
        self.shared
            .history
            .lock()
            .await
            .recent(n)
            .cloned()
            .collect()
    }

    pub async fn history_search(
        &self,
        query: String,
        limit: usize,
    ) -> Vec<crate::history::HistoryEntry> {
        self.shared
            .history
            .lock()
            .await
            .search_fulltext(&query)
            .take(limit)
            .cloned()
            .collect()
    }

    pub async fn history_repush(&self, id: u64, target: SyncTarget) -> Result<()> {
        let entry = self
            .shared
            .history
            .lock()
            .await
            .get(id)
            .cloned()
            .context("history entry not found")?;
        if let crate::history::HistoryPayload::Text {
            full_text, preview, ..
        } = entry.payload
        {
            let text = full_text.unwrap_or(preview);
            self.push_clipboard_to(ClipboardContent::Text(text), target)
                .await;
        }
        Ok(())
    }

    pub async fn history_set_pinned(&self, id: u64, pinned: bool) -> Result<()> {
        self.shared.history.lock().await.set_pinned(id, pinned)?;
        Ok(())
    }

    pub async fn history_delete(&self, id: u64) -> Result<bool> {
        self.shared.history.lock().await.remove(id)
    }

    pub async fn history_clear(&self) -> Result<()> {
        self.shared.history.lock().await.clear()
    }

    pub async fn history_export_csv(&self) -> String {
        self.shared.history.lock().await.export_csv()
    }

    pub async fn history_export_json(&self) -> Result<String> {
        self.shared.history.lock().await.export_json()
    }

    pub async fn history_stats(&self) -> crate::history::HistoryStats {
        self.shared.history.lock().await.stats()
    }

    pub async fn history_add_tag(&self, id: u64, tag: String) -> Result<()> {
        self.shared.history.lock().await.add_tag(id, &tag)?;
        Ok(())
    }

    pub async fn history_remove_tag(&self, id: u64, tag: String) -> Result<()> {
        self.shared.history.lock().await.remove_tag(id, &tag)?;
        Ok(())
    }

    pub async fn history_filtered(
        &self,
        kind: Option<String>,
        device: Option<String>,
        from_secs: Option<u64>,
        to_secs: Option<u64>,
        tag: Option<String>,
        limit: usize,
        pinned_only: bool,
    ) -> Vec<crate::history::HistoryEntry> {
        let filter = crate::history::HistoryFilter {
            kind,
            device,
            from_secs,
            to_secs,
            tag,
            limit: Some(limit),
            pinned_only,
        };
        self.shared
            .history
            .lock()
            .await
            .filter(&filter)
            .take(limit)
            .cloned()
            .collect()
    }

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
            .map(|e| serde_json::to_value(e).ok())
            .flatten()
    }

    // ── Templates ─────────────────────────────────────────────────────────────

    pub async fn template_list(&self) -> Vec<crate::settings::ClipboardTemplate> {
        self.shared
            .settings
            .lock()
            .await
            .clipboard_templates
            .clone()
    }

    pub async fn template_push(&self, name: String, target: SyncTarget) -> Result<()> {
        let templates = self
            .shared
            .settings
            .lock()
            .await
            .clipboard_templates
            .clone();
        let tmpl = templates
            .iter()
            .find(|t| t.name == name)
            .cloned()
            .with_context(|| format!("template '{}' not found", name))?;
        let content = crate::protocol::ClipboardContent::Text(tmpl.text);
        match target {
            SyncTarget::All => {
                self.push_clipboard(content).await;
            }
            SyncTarget::Device(id) => {
                self.push_clipboard_to(content, SyncTarget::Device(id))
                    .await;
            }
        }
        Ok(())
    }

    pub async fn template_set(
        &self,
        name: String,
        text: String,
        description: String,
    ) -> Result<()> {
        let mut settings = self.shared.settings.lock().await;
        if let Some(t) = settings
            .clipboard_templates
            .iter_mut()
            .find(|t| t.name == name)
        {
            t.text = text;
            t.description = description;
        } else {
            settings
                .clipboard_templates
                .push(crate::settings::ClipboardTemplate {
                    name,
                    text,
                    description,
                });
        }
        Ok(())
    }

    pub async fn template_remove(&self, name: String) -> Result<bool> {
        let mut settings = self.shared.settings.lock().await;
        let before = settings.clipboard_templates.len();
        settings.clipboard_templates.retain(|t| t.name != name);
        Ok(settings.clipboard_templates.len() < before)
    }

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

    // ── Feedback ──────────────────────────────────────────────────────────────

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
            let peers = self.shared.peer_manager.active_senders();
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
        let peers = self.shared.peer_manager.active_senders();
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
                warn!("file transfer announce queue unavailable for peer {}", peer_id);
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

        self.shared.activity.lock().await.record_file_transfer_started(
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
            let peers = self.shared.peer_manager.active_senders();
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
        let peers = self.shared.peer_manager.active_senders();
        for (_, tx) in peers {
            let _ = tx.try_send(cancel_msg.clone());
        }
        Ok(())
    }

    /// Trigger a fresh mDNS browse query without restarting the advertisement.
    /// Called by the Mac "Scan" button — surfaces peers that came online
    /// since the last browse without a full discovery restart.
    pub async fn rescan_peers(&self) {
        if let Some(tx) = &self.shared.discovery_tx {
            let _ = tx.send(DiscoveryCommand::Rescan).await;
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

    /// Push the current OS clipboard content to connected peers.
    /// The daemon reads the clipboard via the platform clipboard API.
    pub async fn push_current_clipboard(&self, target: SyncTarget) -> Result<()> {
        let text = self
            .shared
            .local_clipboard
            .lock()
            .await
            .read_text()
            .context("reading local clipboard")?;
        if let Some(text) = text {
            self.push_clipboard_to(ClipboardContent::Text(text), target)
                .await;
        }
        Ok(())
    }

    /// Returns this engine's stable device UUID.
    /// Used by the Android JNI bridge to filter out self-connections during NSD.
    pub async fn device_id(&self) -> Uuid {
        self.shared.config.device_id
    }

    /// Returns this device's Noise public-key fingerprint as a lowercase hex string.
    /// Displayed in the Mac Security pane and Android pairing screen for manual verification.
    pub fn local_fingerprint(&self) -> String {
        self.shared
            .identity_pubkey
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

    pub async fn connect_to_peer(&self, ip: String, port: u16) -> Result<()> {
        let addr = SocketAddr::new(ip.parse().context("invalid peer IP")?, port);
        self.shared.peer_manager.note_manual_target(addr);
        match connect_once(self.shared.clone(), addr, None, DiscoverySource::Manual).await {
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

    /// Pause Sync: keep connection alive, suppress clipboard data flow.
    pub async fn pause_sync_peer(&self, device_id: Uuid) -> Result<bool> {
        self.shared.peer_manager.set_sync_enabled(device_id, false)
    }

    /// Resume Sync: re-enable clipboard data flow.
    pub async fn resume_sync_peer(&self, device_id: Uuid) -> Result<bool> {
        self.shared.peer_manager.set_sync_enabled(device_id, true)
    }

    /// Forget Device: remove persistent pairing without revoking trust.
    pub async fn forget_device(&self, device_id: Uuid) -> Result<bool> {
        let found = self.shared.peer_manager.forget_device(device_id)?;
        if found {
            // Disconnect the session — device will not auto-reconnect
            let session = self.shared.peer_manager.shutdown_peer_session(device_id)?;
            if let Some(session) = session {
                if let Some(shutdown_tx) = session.shutdown_tx {
                    let _ = shutdown_tx.send(crate::peer_manager::SessionShutdown {
                        reason: "device forgotten".to_string(),
                        send_bye: true,
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

    /// Spawn a background task that periodically prunes transient, untrusted
    /// peer records to prevent unbounded memory/disk growth (MED-05).
    fn spawn_peer_pruner(&self) {
        let peer_manager = self.shared.peer_manager.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
            interval.tick().await; // skip the first immediate tick
            loop {
                interval.tick().await;
                peer_manager.prune_stale_peers();
            }
        });
    }

    fn spawn_sensitive_history_pruner(&self) {
        let history = self.shared.history.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                if let Err(err) = history.lock().await.purge_expired_sensitive_entries() {
                    warn!(error = %err, "sensitive history pruning failed");
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
                        .map(|peer| peer.friendly_name);
                    let _ = peer_shared
                        .peer_manager
                        .mark_disconnected(device_id, Some("mDNS announcement lost".to_string()));
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

                // Rescan: trigger a fresh mDNS browse query without tearing down
                // the advertisement.  On the Mac this is the "Scan" button; on
                // Android the NSD retry scheduler fires this after a reconnect.
                DiscoveryCommand::Rescan => {
                    if let Some(ref discovery) = current {
                        // Re-issue the browse query — causes peers to re-announce.
                        let _ = discovery.browse(peer_tx.clone());
                        tracing::info!("mDNS rescan triggered");
                    } else {
                        tracing::debug!("rescan requested but discovery not running");
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
                tokio::time::sleep(Duration::from_millis(200)).await;
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
                tokio::time::sleep(Duration::from_millis(500)).await;
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

    let sessions = shared.peer_manager.shutdown_all_sessions(&reason)?;
    for session in sessions {
        if let Some(shutdown_tx) = session.shutdown_tx {
            let _ = shutdown_tx.send(SessionShutdown {
                reason: reason.clone(),
                send_bye: false,
            });
        }
    }

    send_listener_rebind(&shared, current_addr).await?;
    if let Some(discovery_tx) = &shared.discovery_tx {
        let _ = discovery_tx
            .send(DiscoveryCommand::Restart {
                bind_ip: current_addr.ip(),
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
            peer.ip,
            shared.config.device_id,
            &shared.config.device_name,
            Some(local_ip),
        ) {
            continue;
        }

        if !peer.should_auto_reconnect() && peer.discovery != DiscoverySource::Manual {
            continue;
        }

        if let Some(endpoint) = peer.socket_addr() {
            if !should_initiate_session(&shared, peer.id, peer.discovery) {
                continue;
            }
            scheduled.insert(endpoint);
            let shared_clone = shared.clone();
            tokio::spawn(async move {
                if let Err(err) =
                    connect_loop(shared_clone, endpoint, Some(peer.id), peer.discovery).await
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
                    connect_loop(shared_clone, endpoint, None, DiscoverySource::Manual).await
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
                connect_loop(shared_clone, endpoint, None, DiscoverySource::Manual).await
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
            peer.ip,
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

    let mut peers: Vec<_> = deduped.into_values().collect();
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

    match (peer_ip, local_ip) {
        (Some(peer_ip), Some(local_ip))
            if peer_ip == local_ip
                && peer_name.trim().eq_ignore_ascii_case(local_device_name.trim()) =>
        {
            true
        }
        _ => false,
    }
}

fn peer_should_replace(current: &PeerRecord, candidate: &PeerRecord) -> bool {
    peer_display_rank(candidate) < peer_display_rank(current)
}

fn peer_display_rank(peer: &PeerRecord) -> (u8, u8, u8, std::cmp::Reverse<u64>, std::cmp::Reverse<u64>) {
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
    let addr = SocketAddr::new(peer.addr, peer.port);
    let trusted = shared.trust.lock().await.is_trusted(peer.device_id);
    shared.peer_manager.upsert_peer(
        peer.device_id,
        peer.device_name.clone(),
        addr,
        trusted,
        DiscoverySource::Mdns,
    )?;

    if !should_initiate_session(&shared, peer.device_id, DiscoverySource::Mdns) {
        return Ok(());
    }

    if shared.peer_manager.live_endpoint(peer.device_id) == Some(addr) {
        return Ok(());
    }

    if matches!(
        shared.peer_manager.get(peer.device_id),
        Some(record)
            if record.status == PeerConnectionState::Connecting
                && record.socket_addr() == Some(addr)
    ) {
        return Ok(());
    }

    let shared_clone = shared.clone();
    tokio::spawn(async move {
        if let Err(err) = connect_loop(
            shared_clone,
            addr,
            Some(peer.device_id),
            DiscoverySource::Mdns,
        )
        .await
        {
            warn!(peer_id = %peer.device_id, error = %err, "discovered peer connection failed");
        }
    });

    Ok(())
}

async fn handle_incoming(shared: EngineShared, mut stream: TcpStream) -> Result<()> {
    network::optimize_stream(&stream, "incoming engine stream");
    let hs = network::handshake_responder(
        &mut stream,
        shared.config.device_id,
        &shared.config.device_name,
        shared.identity_pubkey,
        false,
    )
    .await?;

    if shared
        .peer_manager
        .is_explicitly_disconnected(hs.peer_device_id)
    {
        anyhow::bail!(
            "ignoring inbound session from {} because it was explicitly disconnected",
            hs.peer_device_id
        );
    }

    let endpoint = stream.peer_addr().context("reading remote address")?;
    let trusted = observe_trust(
        &shared,
        hs.peer_device_id,
        hs.peer_device_name.clone(),
        hs.peer_identity_pubkey_bytes,
    )
    .await?;

    shared.peer_manager.upsert_peer(
        hs.peer_device_id,
        hs.peer_device_name.clone(),
        endpoint,
        trusted,
        DiscoverySource::Mdns,
    )?;

    register_session(
        shared,
        stream,
        endpoint,
        hs.peer_device_id,
        hs.peer_device_name,
        hs.session,
        trusted,
        DiscoverySource::Mdns,
    )
}

async fn connect_loop(
    shared: EngineShared,
    endpoint: SocketAddr,
    expected_device_id: Option<Uuid>,
    discovery: DiscoverySource,
) -> Result<()> {
    if let Some(device_id) = expected_device_id {
        if !shared
            .peer_manager
            .mark_connecting(device_id, Some(endpoint))?
        {
            return Ok(());
        }
    }

    let mut backoff = Backoff::new(endpoint.to_string());
    loop {
        match connect_once(shared.clone(), endpoint, expected_device_id, discovery).await {
            Ok(()) => {
                shared.peer_manager.clear_manual_target(endpoint);
                return Ok(());
            }
            Err(err) => {
                if let Some(device_id) = expected_device_id {
                    let _ = shared
                        .peer_manager
                        .mark_failed(device_id, endpoint, err.to_string());
                } else {
                    shared.peer_manager.record_manual_failure(endpoint);
                }

                match backoff.next() {
                    Some(delay) => {
                        warn!(addr = %endpoint, error = %err, retry_in_ms = delay.as_millis(), "peer connect failed");
                        tokio::time::sleep(delay).await;
                    }
                    None => {
                        let message =
                            format!("connection to {endpoint} failed after retries: {err}");
                        let _ = shared.event_tx.send(EngineEvent::Warning(message)).await;
                        return Err(err);
                    }
                }
            }
        }
    }
}

async fn connect_once(
    shared: EngineShared,
    endpoint: SocketAddr,
    expected_device_id: Option<Uuid>,
    discovery: DiscoverySource,
) -> Result<()> {
    let started = Instant::now();
    let mut stream = timeout(shared.config.connect_timeout, TcpStream::connect(endpoint))
        .await
        .context("connect timeout")?
        .with_context(|| format!("connecting to {endpoint}"))?;
    network::optimize_stream(&stream, "outgoing engine stream");

    let hs = network::handshake_initiator(
        &mut stream,
        shared.config.device_id,
        &shared.config.device_name,
        shared.identity_pubkey,
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

    let trusted = observe_trust(
        &shared,
        hs.peer_device_id,
        hs.peer_device_name.clone(),
        hs.peer_identity_pubkey_bytes,
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

    register_session(
        shared,
        stream,
        endpoint,
        hs.peer_device_id,
        hs.peer_device_name,
        hs.session,
        trusted,
        discovery,
    )
}

async fn observe_trust(
    shared: &EngineShared,
    device_id: Uuid,
    device_name: String,
    identity_pubkey: [u8; 32],
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
            let _ = shared
                .event_tx
                .send(EngineEvent::TofuPrompt {
                    device_id,
                    device_name,
                    fingerprint_display: format_fingerprint(&record.key_fingerprint),
                })
                .await;
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
) -> Result<()> {
    let (outbox_tx, mut outbox_rx) = mpsc::channel::<AppMessage>(256);
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<SessionShutdown>();
    shared
        .peer_manager
        .upsert_peer(peer_id, peer_name.clone(), endpoint, trusted, discovery)?;
    let _ = shared
        .peer_manager
        .set_explicit_disconnect(peer_id, false);
    let (session_id, replaced) = shared.peer_manager.replace_live_session(
        peer_id,
        endpoint,
        outbox_tx.clone(),
        shutdown_tx,
    )?;

    if let Some(replaced) = replaced {
        if let Some(old_shutdown) = replaced.shutdown_tx {
            let _ = old_shutdown.send(SessionShutdown {
                reason: format!("session migrated to {}", endpoint),
                send_bye: true,
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
    let session_handle = tokio::spawn(async move {
        let mut sess = PeerSession {
            stream,
            session,
            peer_device_id: peer_id,
            peer_device_name: peer_name.clone(),
        };
        let mut heartbeat = tokio::time::interval(shared.config.heartbeat_interval);
        let mut last_seen = Instant::now();
        // Tracks when we sent the most recent Ping so Pong receipt gives an
        // accurate RTT sample for the quality probe (HIGH-03).
        let mut ping_sent_at: Option<Instant> = None;
        let disconnect_reason = loop {
            tokio::select! {
                shutdown = &mut shutdown_rx => {
                    match shutdown {
                        Ok(cmd) => {
                            if cmd.send_bye {
                                let _ = sess.send(&AppMessage::Bye).await;
                            }
                            break cmd.reason;
                        }
                        Err(_) => {
                            break "session shutdown channel dropped".to_string();
                        }
                    }
                }
                _ = heartbeat.tick() => {
                    if last_seen.elapsed() > shared.config.heartbeat_timeout {
                        break "heartbeat timeout".to_string();
                    }
                    // Use probe::make_ping() which embeds a high-resolution
                    // timestamp, and record the send instant for RTT calc (HIGH-03).
                    let ping = probe::make_ping();
                    ping_sent_at = Some(Instant::now());
                    if let Err(err) = sess.send(&ping).await {
                        break format!("heartbeat send failed: {err}");
                    }
                }
                Some(msg) = outbox_rx.recv() => {
                    if let Err(err) = sess.send(&msg).await {
                        break format!("send failed: {err}");
                    }
                }
                result = sess.recv() => {
                    match result {
                        Ok(AppMessage::ClipboardPush { seq, content, origin_device, origin_device_name, relay_path }) => {
                            last_seen = Instant::now();
                            if shared.peer_manager.get(peer_id).map(|peer| peer.is_sync_eligible()).unwrap_or(false) {
                                let _ = shared.peer_manager.update_last_sync(peer_id);
                                let display_name = if origin_device_name.is_empty() {
                                    peer_name.clone()
                                } else {
                                    origin_device_name.clone()
                                };

                                // ── Timeline-first clipboard UX ───────────────
                                let hash = hash_content(&content);
                                let hash_hex = hex::encode(hash);
                                let auto_apply = shared.apply_policy.lock().await
                                    .should_auto_apply(origin_device);

                                // Record in activity feed.
                                let activity_id = {
                                    if let ClipboardContent::Text(ref text) = content {
                                        shared
                                            .clipboard_store
                                            .lock()
                                            .await
                                            .insert(hash_hex.clone(), text.clone());
                                    }
                                    let mut feed = shared.activity.lock().await;
                                    if let ClipboardContent::Text(ref text) = content {
                                        feed.record_remote_clipboard_text(
                                            origin_device,
                                            display_name.clone(),
                                            text,
                                            hash_hex.clone(),
                                            relay_path.clone(),
                                        )
                                    } else {
                                        feed.record_remote_clipboard_image(
                                            origin_device,
                                            display_name.clone(),
                                            "image",
                                            0,
                                            hash_hex.clone(),
                                            relay_path.clone(),
                                        )
                                    }
                                };

                                // If auto-applying, mark immediately applied.
                                if auto_apply {
                                    let mut feed = shared.activity.lock().await;
                                    feed.record_clipboard_applied(origin_device, display_name.clone(), hash_hex.clone());
                                }

                                // Wrap content in Arc here so all downstream users — the
                                // EngineEvent and every relay-fanout hop — share one heap
                                // allocation instead of N independent clones (MED-01).
                                let shared_content = std::sync::Arc::new(content);

                                let _ = shared.event_tx.send(EngineEvent::ClipboardReceived {
                                    from_device: origin_device,
                                    from_name: display_name.clone(),
                                    content: (*shared_content).clone(),
                                    auto_applied: auto_apply,
                                    relay_path: relay_path.clone(),
                                    activity_id,
                                }).await;
                                let _ = sess.send(&AppMessage::ClipboardAck { seq }).await;

                                // Persist the incoming item to history.
                                {
                                    let max_bytes = shared.settings.lock().await.max_history_text_bytes;
                                    let source = display_name.clone();
                                    let _ = shared.history.lock().await
                                        .push_with_options(&(*shared_content), source, max_bytes);
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
                                    if fp_id == peer_id { continue; }
                                    let Some(fp) = shared.peer_manager.get(fp_id) else { continue; };
                                    if !fp.is_sync_eligible() { continue; }
                                    if !router.should_relay_to(hash, origin_device, fp_id, &relay_path) { continue; }
                                    let mut extended_path = relay_path.clone();
                                    extended_path.push(shared.config.device_name.clone());
                                    let _ = fp_tx.try_send(AppMessage::ClipboardPush {
                                        seq,
                                        content: (*shared_content).clone(),
                                        origin_device,
                                        origin_device_name: display_name.clone(),
                                        relay_path: extended_path,
                                    });
                                }
                            } else {
                                let _ = shared.event_tx.send(EngineEvent::Warning(format!(
                                    "ignoring clipboard payload from untrusted/paused peer {}",
                                    peer_name
                                ))).await;
                            }
                        }
                        Ok(AppMessage::FileTransferAnnounce { meta }) => {
                            last_seen = Instant::now();
                            let transfer_id = meta.transfer_id;
                            let file_name = meta.file_name.clone();
                            let file_bytes = meta.size_bytes;
                            let mime_type = meta.mime_type.clone();

                            // Register inbound transfer.
                            shared.file_transfers.lock().await
                                .register_inbound(meta, peer_id, peer_name.clone());

                            // Check auto-accept policy.
                            let settings = shared.settings.lock().await.clone();
                            let auto_accept = settings.auto_accept_file_transfers
                                && (settings.auto_accept_max_bytes == 0 || file_bytes <= settings.auto_accept_max_bytes)
                                && shared.peer_manager.get(peer_id).map(|p| p.trusted).unwrap_or(false);

                            if auto_accept {
                                let resume_from = shared.file_transfers.lock().await
                                    .accept_inbound_or_resume(&transfer_id).unwrap_or(0);
                                let _ = sess.send(&AppMessage::FileTransferAccept {
                                    transfer_id,
                                    accepted: true,
                                    resume_from_chunk: resume_from,
                                    reject_reason: None,
                                }).await;
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
                                let _ = shared.event_tx.send(EngineEvent::FileTransferIncoming {
                                    transfer_id,
                                    from_device: peer_id,
                                    from_name: peer_name.clone(),
                                    file_name,
                                    file_bytes,
                                    mime_type,
                                }).await;
                            }
                        }
                        Ok(AppMessage::FileTransferAccept { transfer_id, accepted, resume_from_chunk, reject_reason }) => {
                            last_seen = Instant::now();
                            if !accepted {
                                shared.file_transfers.lock().await.cancel_outbound(&transfer_id);
                                let _ = shared.event_tx.send(EngineEvent::FileTransferFailed {
                                    transfer_id,
                                    from_device: peer_id,
                                    reason: reject_reason.unwrap_or_else(|| "rejected".into()),
                                }).await;
                            } else {
                                {
                                    let mut mgr = shared.file_transfers.lock().await;
                                    if let Some(transfer) = mgr.get_outbound_mut(&transfer_id) {
                                        transfer.resume_from(resume_from_chunk);
                                    }
                                }

                                let mut stream_failed = false;
                                loop {
                                    let next_chunk = {
                                        let mut mgr = shared.file_transfers.lock().await;
                                        match mgr.get_outbound_mut(&transfer_id) {
                                            Some(transfer) => transfer.next_chunk_message(),
                                            None => Ok(None),
                                        }
                                    };

                                    let Some(chunk_msg) = (match next_chunk {
                                        Ok(value) => value,
                                        Err(err) => {
                                            warn!(error = %err, "failed to read outbound file chunk");
                                            shared.file_transfers.lock().await.cancel_outbound(&transfer_id);
                                            stream_failed = true;
                                            break;
                                        }
                                    }) else {
                                        break;
                                    };

                                    let wire_msg = match chunk_msg {
                                        FileTransferMessage::Chunk {
                                            transfer_id,
                                            chunk_index,
                                            total_chunks,
                                            data,
                                        } => AppMessage::FileChunk {
                                            transfer_id,
                                            chunk_index,
                                            total_chunks,
                                            data,
                                        },
                                        _ => continue,
                                    };
                                    if let Err(e) = sess.send(&wire_msg).await {
                                        warn!("file chunk send error: {}", e);
                                        stream_failed = true;
                                        break;
                                    }
                                }

                                if !stream_failed {
                                    let all_sent = {
                                        let mut mgr = shared.file_transfers.lock().await;
                                        mgr.get_outbound_mut(&transfer_id)
                                            .map(|transfer| transfer.is_all_sent())
                                            .unwrap_or(false)
                                    };
                                    if all_sent {
                                        let _ = sess.send(&AppMessage::FileTransferComplete { transfer_id }).await;
                                    }
                                }
                            }
                        }
                        Ok(AppMessage::FileChunk { transfer_id, chunk_index, total_chunks: _, data }) => {
                            last_seen = Instant::now();
                            let (progress, should_ack) = {
                                let mut mgr = shared.file_transfers.lock().await;
                                if let Some(transfer) = mgr.get_inbound_mut(&transfer_id) {
                                    let prog = transfer.receive_chunk(chunk_index, data).ok();
                                    let ack = transfer.should_ack();
                                    (prog, ack)
                                } else {
                                    (None, false)
                                }
                            };
                            if let Some(prog) = progress {
                                let from_device = peer_id;
                                let file_name = shared.file_transfers.lock().await
                                    .get_inbound_mut(&transfer_id)
                                    .map(|t| t.meta.file_name.clone())
                                    .unwrap_or_default();
                                let _ = shared.event_tx.send(EngineEvent::FileTransferProgress {
                                    transfer_id,
                                    from_device,
                                    file_name,
                                    percent: prog.percent,
                                    bytes_received: prog.bytes_received,
                                    total_bytes: prog.total_bytes,
                                    speed_bps: prog.speed_bps,
                                    eta_secs: prog.eta_secs,
                                }).await;
                            }
                            if should_ack {
                                let last_confirmed = shared.file_transfers.lock().await
                                    .get_inbound_mut(&transfer_id)
                                    .map(|t| t.last_confirmed_chunk)
                                    .unwrap_or(0);
                                let _ = sess.send(&AppMessage::FileChunkAck {
                                    transfer_id,
                                    last_confirmed_chunk: last_confirmed,
                                }).await;
                            }
                        }
                        Ok(AppMessage::FileChunkAck { transfer_id, last_confirmed_chunk }) => {
                            last_seen = Instant::now();
                            if let Some(transfer) = shared.file_transfers.lock().await.get_outbound_mut(&transfer_id) {
                                transfer.on_chunk_ack(last_confirmed_chunk);
                            }
                        }
                        Ok(AppMessage::FileTransferComplete { transfer_id }) => {
                            last_seen = Instant::now();
                            // Finalize: verify SHA-256 and write to disk.
                            let result = {
                                let mut mgr = shared.file_transfers.lock().await;
                                if let Some(transfer) = mgr.get_inbound_mut(&transfer_id) {
                                    let file_name = transfer.meta.file_name.clone();
                                    let file_bytes = transfer.meta.size_bytes;
                                    match transfer.finalize() {
                                        Ok(dest) => Ok((dest, file_name, file_bytes)),
                                        Err(e) => Err(e.to_string()),
                                    }
                                } else {
                                    Err("transfer not found".into())
                                }
                            };
                            match result {
                                Ok((dest, file_name, file_bytes)) => {
                                    shared.file_transfers.lock().await.remove_inbound(&transfer_id);
                                    let hex_tid = hex::encode(transfer_id);
                                    let dest_path_str = dest.to_string_lossy().to_string();
                                    shared.activity.lock().await.record_file_transfer_complete(
                                        peer_id, peer_name.clone(), file_name.clone(), file_bytes,
                                        hex_tid, Some(dest_path_str)
                                    );
                                    let _ = sess.send(&AppMessage::FileTransferCompleteAck {
                                        transfer_id,
                                        success: true,
                                        error: None,
                                    }).await;
                                    let _ = shared.event_tx.send(EngineEvent::FileTransferComplete {
                                        transfer_id,
                                        from_device: peer_id,
                                        from_name: peer_name.clone(),
                                        file_name,
                                        dest_path: dest,
                                    }).await;
                                }
                                Err(e) => {
                                    let hex_tid = hex::encode(transfer_id);
                                    shared.activity.lock().await.record_file_transfer_failed(
                                        peer_id, peer_name.clone(), None, hex_tid, e.clone()
                                    );
                                    let _ = sess.send(&AppMessage::FileTransferCompleteAck {
                                        transfer_id,
                                        success: false,
                                        error: Some(e.clone()),
                                    }).await;
                                    let _ = shared.event_tx.send(EngineEvent::FileTransferFailed {
                                        transfer_id,
                                        from_device: peer_id,
                                        reason: e,
                                    }).await;
                                }
                            }
                        }
                        Ok(AppMessage::FileTransferCompleteAck { transfer_id, success: _, error: _ }) => {
                            last_seen = Instant::now();
                            shared.file_transfers.lock().await.remove_outbound(&transfer_id);
                        }
                        Ok(AppMessage::FileTransferCancel { transfer_id, reason }) => {
                            last_seen = Instant::now();
                            {
                                let mut mgr = shared.file_transfers.lock().await;
                                mgr.cancel_inbound(&transfer_id, &reason);
                                mgr.cancel_outbound(&transfer_id);
                            }
                            let _ = shared.event_tx.send(EngineEvent::FileTransferFailed {
                                transfer_id,
                                from_device: peer_id,
                                reason,
                            }).await;
                        }
                        Ok(AppMessage::HistoryMetadata { entry }) => {
                            last_seen = Instant::now();
                            let _ = shared.peer_manager.update_last_sync(peer_id);
                            let _ = shared.event_tx.send(EngineEvent::HistoryMetadataReceived {
                                from_device: peer_id,
                                from_name: peer_name.clone(),
                                entry,
                            }).await;
                        }
                        Ok(AppMessage::ClipboardAck { seq }) => {
                            last_seen = Instant::now();
                            let _ = shared.peer_manager.update_last_sync(peer_id);
                            let _ = shared.event_tx.send(EngineEvent::ClipboardSynced {
                                peer_device: peer_id,
                                peer_name: peer_name.clone(),
                                seq,
                            }).await;
                        }
                        Ok(AppMessage::Ping { timestamp_ms }) => {
                            last_seen = Instant::now();
                            let _ = sess.send(&AppMessage::Pong { timestamp_ms }).await;
                        }
                        Ok(AppMessage::Pong { timestamp_ms: _ }) => {
                            last_seen = Instant::now();
                            // Feed the RTT sample into the peer's quality probe
                            // using the Instant captured at send time, which is
                            // far more accurate than round-tripping wall-clock ms
                            // over the network (HIGH-03).
                            if let Some(sent_at) = ping_sent_at.take() {
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
                            state, number, contact_name,
                            origin_device, origin_device_name,
                        }) => {
                            last_seen = Instant::now();
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
                            let _ = shared.event_tx.send(EngineEvent::CallStateChanged {
                                from_device: origin_device,
                                from_name: origin_device_name,
                                state,
                                number,
                                contact_name,
                            }).await;
                        }
                        Ok(AppMessage::BatteryStatus {
                            level,
                            charging,
                            origin_device,
                            origin_device_name,
                        }) => {
                            last_seen = Instant::now();
                            // Persist in shared state for IPC status polling.
                            {
                                let mut batteries = shared.peer_batteries.lock().await;
                                batteries.insert(origin_device, PeerBatteryState {
                                    device_id: origin_device,
                                    device_name: origin_device_name.clone(),
                                    level,
                                    charging,
                                });
                            }
                            let _ = shared.event_tx.send(EngineEvent::BatteryStateChanged {
                                from_device: origin_device,
                                from_name: origin_device_name,
                                level,
                                charging,
                            }).await;
                        }
                        Ok(AppMessage::CallAction { action, origin_device }) => {
                            last_seen = Instant::now();
                            tracing::info!("Received CallAction: {} from {:?}", action, origin_device);
                            let _ = shared.event_tx.send(EngineEvent::CallActionRequest {
                                action,
                                from_device: origin_device,
                            }).await;
                        }
                        Ok(AppMessage::Bye) => {
                            let _ = shared.peer_manager.set_explicit_disconnect(peer_id, true);
                            break "peer closed session".to_string();
                        }
                        Err(err) => {
                            break err.to_string();
                        }
                    }
                }
            }
        };

        let reason = Some(disconnect_reason);
        match shared
            .peer_manager
            .mark_disconnected_if_current(peer_id, session_id, reason.clone())
        {
            Ok(true) => {
                tracing::warn!("peer disconnected: peer_id={}, reason={:?}", peer_id, reason);
                let _ = shared
                    .event_tx
                    .send(EngineEvent::PeerDisconnected {
                        device_id: peer_id,
                        device_name: Some(peer_name.clone()),
                        reason: reason.clone(),
                    })
                    .await;

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
                    .map(|peer| peer.trusted || peer.discovery == DiscoverySource::Manual)
                    .unwrap_or(false)
                {
                    if !should_initiate_session(&shared, peer_id, discovery) {
                        return;
                    }
                    if let Some(reconnect_endpoint) = shared.peer_manager.endpoint_for(peer_id) {
                        let shared_clone = shared.clone();
                        tokio::spawn(async move {
                            if let Err(err) = connect_loop(
                                shared_clone,
                                reconnect_endpoint,
                                Some(peer_id),
                                discovery,
                            )
                            .await
                            {
                                warn!(peer_id = %peer_id, error = %err, "reconnect failed");
                            }
                        });
                    }
                }
            }
            Ok(false) => {}
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

fn should_initiate_session(
    shared: &EngineShared,
    peer_id: Uuid,
    discovery: DiscoverySource,
) -> bool {
    if shared.peer_manager.is_explicitly_disconnected(peer_id) {
        return false;
    }
    match discovery {
        DiscoverySource::Manual => true,
        DiscoverySource::Mdns | DiscoverySource::Unknown => {
            shared.config.device_id.as_bytes() < peer_id.as_bytes()
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
