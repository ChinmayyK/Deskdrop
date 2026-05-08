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
use crate::protocol::{AppMessage, ClipboardContent, HistoryMetadata, DEFAULT_PORT};
use crate::retry::Backoff;
use crate::settings::{default_peer_store_path, default_trust_store_path, Settings};
use crate::trust::{format_fingerprint, TrustRecord, TrustState, TrustStore};
use anyhow::{Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
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
    Restart { bind_ip: IpAddr, port: u16 },
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
            file_transfers: Arc::new(Mutex::new(FileTransferManager::new(default_save_dir()))),
            apply_policy: Arc::new(Mutex::new(ClipboardApplyPolicy::default())),
            settings: Arc::new(Mutex::new(Settings::default())),
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
        Ok(engine)
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
            *guard += 1;
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
        self.shared.activity.lock().await.recent(limit).into_iter().cloned().collect()
    }

    /// Get activity feed entries added after `since_id`.
    pub async fn activity_since(&self, since_id: u64) -> Vec<crate::activity::ActivityEntry> {
        self.shared.activity.lock().await.since(since_id).into_iter().cloned().collect()
    }

    /// Get pending remote clipboard items not yet applied locally.
    pub async fn pending_remote_clipboards(&self) -> Vec<crate::activity::ActivityEntry> {
        self.shared.activity.lock().await.pending_remote_clipboards().into_iter().cloned().collect()
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
        let Some(entry) = entry else { return Ok(false); };
        let from_device = entry.device_id;
        let from_name = entry.device_name.clone();
        {
            let mut feed = self.shared.activity.lock().await;
            feed.record_clipboard_applied(from_device, from_name.clone(), content_hash);
        }
        // Emit event so the platform layer writes to local clipboard.
        let _ = self.shared.event_tx.send(EngineEvent::ClipboardReceived {
            from_device,
            from_name,
            content: ClipboardContent::Text(entry.text_preview.unwrap_or_default()),
            auto_applied: true,
            relay_path: entry.relay_path,
            activity_id: entry.id,
        }).await;
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

    // ── File Transfer ─────────────────────────────────────────────────────────

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
        drop(transfer); // release immutable borrow before mutable use

        // Announce to target peer(s).
        let announce = AppMessage::FileTransferAnnounce { meta };
        let peers = self.shared.peer_manager.active_senders();
        for (peer_id, tx) in peers {
            let should_send = match target_device {
                Some(t) => t == peer_id,
                None => true,
            };
            if should_send {
                let _ = tx.try_send(announce.clone());
            }
        }
        drop(mgr);

        // Record in activity feed.
        {
            let mut feed = self.shared.activity.lock().await;
            feed.record_file_transfer_started(
                self.shared.config.device_id,
                self.shared.config.device_name.clone(),
                file_name,
                size_bytes,
                hex::encode(transfer_id),
                true,
            );
        }
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

    /// Reject an incoming file transfer.
    pub async fn reject_file_transfer(&self, transfer_id: [u8; 16], reason: String) -> Result<()> {
        let from_device = {
            let mut mgr = self.shared.file_transfers.lock().await;
            let dev = mgr.all_inbound()
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

    pub async fn connect_to_peer(&self, ip: String, port: u16) -> Result<()> {
        let addr = SocketAddr::new(ip.parse().context("invalid peer IP")?, port);
        self.shared.peer_manager.note_manual_target(addr);
        connect_loop(self.shared.clone(), addr, None, DiscoverySource::Manual).await
    }

    pub async fn disconnect_peer(&self, device_id: Uuid) -> Result<bool> {
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
        self.shared.peer_manager.set_auto_connect(device_id, enabled)
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
            peers: self.shared.peer_manager.list(),
            last_sync_at: self.shared.peer_manager.last_sync_at(),
        }
    }

    async fn spawn_network_monitor(&self) -> Result<()> {
        let mut changes = network_manager::spawn_network_monitor(
            self.shared.config.bind_ip,
            self.shared.config.port,
            self.shared.config.network_poll_interval,
        )?;
        let shared = self.shared.clone();

        tokio::spawn(async move {
            while let Some(change) = changes.recv().await {
                if let Err(err) = handle_network_change(shared.clone(), change).await {
                    warn!(error = %err, "network change handling failed");
                }
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
    let peers = shared.peer_manager.list();
    let mut scheduled = HashSet::new();

    for peer in peers {
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
    stream.set_nodelay(true)?;
    let hs = network::handshake_responder(
        &mut stream,
        shared.config.device_id,
        &shared.config.device_name,
        shared.identity_pubkey,
        false,
    )
    .await?;

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
    stream.set_nodelay(true)?;

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

    tokio::spawn(async move {
        let mut sess = PeerSession {
            stream,
            session,
            peer_device_id: peer_id,
            peer_device_name: peer_name.clone(),
        };
        let mut heartbeat = tokio::time::interval(shared.config.heartbeat_interval);
        let mut last_seen = Instant::now();
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
                    let ping = AppMessage::Ping { timestamp_ms: now_secs() * 1000 };
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

                                let _ = shared.event_tx.send(EngineEvent::ClipboardReceived {
                                    from_device: origin_device,
                                    from_name: display_name.clone(),
                                    content: content.clone(),
                                    auto_applied: auto_apply,
                                    relay_path: relay_path.clone(),
                                    activity_id,
                                }).await;
                                let _ = sess.send(&AppMessage::ClipboardAck { seq }).await;

                                // ── Mesh fanout relay ──────────────────────────
                                // If we received from a direct peer but there are other
                                // peers in the mesh, relay onwards (excluding origin + seen).
                                let fanout_peers = shared.peer_manager.active_senders();
                                let mut router = shared.mesh_router.lock().await;
                                for (fp_id, fp_tx) in fanout_peers {
                                    if fp_id == peer_id { continue; }
                                    let Some(fp) = shared.peer_manager.get(fp_id) else { continue; };
                                    if !fp.is_sync_eligible() { continue; }
                                    if !router.should_relay_to(hash, origin_device, fp_id, &relay_path) { continue; }
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
                                // Collect chunks while holding the lock, then drop lock before sending.
                                let (chunks_to_send, all_sent) = {
                                    let mut mgr = shared.file_transfers.lock().await;
                                    if let Some(transfer) = mgr.get_outbound_mut(&transfer_id) {
                                        transfer.resume_from(resume_from_chunk);
                                        let mut chunks = Vec::new();
                                        while let Some(chunk_msg) = transfer.next_chunk_message() {
                                            chunks.push(chunk_msg);
                                        }
                                        let done = transfer.is_all_sent();
                                        (chunks, done)
                                    } else {
                                        (Vec::new(), false)
                                    }
                                }; // lock released here

                                for chunk in chunks_to_send {
                                    if let Err(e) = sess.send(&chunk).await {
                                        warn!("file chunk send error: {}", e);
                                        break;
                                    }
                                }
                                if all_sent {
                                    let _ = sess.send(&AppMessage::FileTransferComplete { transfer_id }).await;
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
                                    shared.activity.lock().await.record_file_transfer_complete(
                                        peer_id, peer_name.clone(), file_name.clone(), file_bytes, hex_tid
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
                        Ok(AppMessage::Pong { .. }) => {
                            last_seen = Instant::now();
                        }
                        Ok(AppMessage::Bye) => {
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
                    feed.lock().await.record_peer_disconnected(peer_id, name, disc_reason);
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

    Ok(())
}

fn should_initiate_session(
    shared: &EngineShared,
    peer_id: Uuid,
    discovery: DiscoverySource,
) -> bool {
    match discovery {
        DiscoverySource::Manual => true,
        DiscoverySource::Mdns | DiscoverySource::Unknown => {
            shared.config.device_id.as_bytes() < peer_id.as_bytes()
        }
    }
}

fn resolve_bind_address(
    config: &EngineConfig,
) -> Result<(Option<NetworkInterfaceInfo>, SocketAddr)> {
    let snapshot = network_manager::resolve_snapshot(config.bind_ip, config.port)?;
    Ok((snapshot.active_interface, snapshot.bind_addr))
}

fn ensure_parent(path: &PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {:?}", parent))?;
    }
    Ok(())
}
