//! Deskdrop peer manager — device lifecycle + session registry.
//!
//! Device state model (layered):
//!
//! ```text
//! Layer          Meaning
//! ─────────────────────────────────────────────────────
//! trusted        Is this device cryptographically allowed?
//! remembered     Is the pairing persisted across restarts?
//! connected      Is there an active TCP session right now?
//! sync_enabled   Should clipboard data flow to/from this peer?
//! auto_connect   Reconnect automatically on startup / network restore?
//! ```

use crate::protocol::AppMessage;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PeerConnectionState {
    Connected,
    #[default]
    Disconnected,
    Connecting,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverySource {
    Mdns,
    Manual,
    UdpBeacon,
    UdpMulticast,
    /// Discovered via hotspot gateway probing (Android/iPhone hotspot).
    HotspotProbe,
    /// Discovered via active outbound TCP connect sweep of the local subnet.
    /// Unlike mDNS/UDP beacons, this never listens for unsolicited inbound
    /// traffic, so it keeps working even when a firewall policy blocks that
    /// (e.g. locked-down corporate networks) without requiring any firewall
    /// exception or admin rights.
    LanProbe,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceLifecycleState {
    Discovered,
    PendingApproval,
    PairingInProgress,
    Paired,
    Trusted,
    Connecting,
    Reconnecting,
    Connected,
    AutoConnected,
}

/// Historical address record for a peer, enabling peer cache probe
/// to try old addresses when a peer moves between IPs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddrRecord {
    pub addr: std::net::SocketAddr,
    pub last_seen_at: u64,
    pub success_count: u32,
}

/// Full device record persisted in the peer store.
///
/// Internal `id` (UUID) is NEVER shown in primary UI.
/// Use `friendly_name` for all user-facing display.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PeerRecord {
    pub id: Uuid,
    pub friendly_name: String,
    pub platform: Option<String>,
    pub ips: Vec<IpAddr>,
    pub port: u16,

    // ── Lifecycle layers ──────────────────────────────────────────────────────
    pub trusted: bool,
    pub remembered: bool,
    pub sync_enabled: bool,
    /// Indicates if the remote peer has globally paused their sync.
    /// Default true (active). If false, UI should show "Sync Paused".
    pub remote_sync_enabled: bool,
    pub auto_connect: bool,

    // ── Runtime state ─────────────────────────────────────────────────────────
    pub status: PeerConnectionState,
    pub last_seen: Option<u64>,
    pub last_sync: Option<u64>,
    pub discovery: DiscoverySource,
    pub last_error: Option<String>,
    /// User manually disconnected this peer and auto-reconnect must stay off
    /// until a fresh, explicit reconnect action is initiated.
    pub explicit_disconnect: bool,
    /// Indicates that this untrusted peer has requested pairing.
    pub pairing_requested: bool,
    /// The generated pairing PIN to display, if pairing is requested.
    pub outgoing_pairing_waiting: bool,
    pub pairing_pin: Option<String>,

    // ── Multi-layer discovery state ──────────────────────────────────────────
    /// When this peer was last seen via any discovery layer (separate from
    /// `last_seen` which tracks TCP session activity).
    #[serde(default)]
    pub last_discovery_at: Option<u64>,
    /// Which discovery layers have seen this peer.
    #[serde(default)]
    pub discovery_sources: Vec<DiscoverySource>,
    /// Historical IP+port records — enables peer cache probe to try old addresses.
    #[serde(default)]
    pub addr_history: Vec<AddrRecord>,
    /// When this peer last disconnected (enables adaptive probe scheduling).
    #[serde(default)]
    pub last_disconnect_at: Option<u64>,

    // ── Computed fields (for UI serialization) ───────────────────────────────
    #[serde(default)]
    pub lifecycle_state: Option<DeviceLifecycleState>,
}

impl Default for PeerRecord {
    fn default() -> Self {
        Self {
            id: Uuid::nil(),
            friendly_name: String::new(),
            platform: None,
            ips: Vec::new(),
            port: crate::protocol::DEFAULT_PORT,
            trusted: false,
            remembered: true,
            sync_enabled: true,
            remote_sync_enabled: true,
            auto_connect: true,
            status: PeerConnectionState::Disconnected,
            last_seen: None,
            last_sync: None,
            discovery: DiscoverySource::Unknown,
            last_error: None,
            explicit_disconnect: false,
            pairing_requested: false,
            outgoing_pairing_waiting: false,
            pairing_pin: None,
            last_discovery_at: None,
            discovery_sources: Vec::new(),
            addr_history: Vec::new(),
            last_disconnect_at: None,
            lifecycle_state: None,
        }
    }
}

impl PeerRecord {
    pub fn lifecycle_state(&self) -> DeviceLifecycleState {
        if self.trusted {
            match self.status {
                PeerConnectionState::Connected => {
                    if self.auto_connect {
                        DeviceLifecycleState::AutoConnected
                    } else {
                        DeviceLifecycleState::Connected
                    }
                }
                PeerConnectionState::Connecting => {
                    if self.explicit_disconnect {
                        DeviceLifecycleState::Connecting
                    } else {
                        DeviceLifecycleState::Reconnecting
                    }
                }
                _ => DeviceLifecycleState::Trusted,
            }
        } else if self.pairing_requested {
            DeviceLifecycleState::PendingApproval
        } else if self.status == PeerConnectionState::Connecting
            || self.status == PeerConnectionState::Connected
            || self.pairing_pin.is_some()
        {
            DeviceLifecycleState::PairingInProgress
        } else {
            DeviceLifecycleState::Discovered
        }
    }

    pub fn socket_addrs(&self) -> Vec<SocketAddr> {
        self.ips
            .iter()
            .map(|ip| SocketAddr::new(*ip, self.port))
            .collect()
    }

    /// Whether this peer should receive clipboard payloads right now.
    pub fn is_sync_eligible(&self) -> bool {
        self.trusted && self.sync_enabled
    }

    /// Whether this peer should reconnect automatically.
    pub fn should_auto_reconnect(&self) -> bool {
        self.trusted && self.remembered && self.auto_connect && !self.explicit_disconnect
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PeerStoreData {
    peers: HashMap<Uuid, PeerRecord>,
}

struct LivePeerSession {
    session_id: u64,
    endpoint: SocketAddr,
    pub sender: mpsc::Sender<AppMessage>,
    pub file_sender: mpsc::Sender<AppMessage>,
    pub shutdown_tx: Option<oneshot::Sender<SessionShutdown>>,
    pub is_outbound: bool,
    pub connected_at: u64,
}

#[derive(Debug)]
pub struct SessionShutdown {
    pub reason: String,
    pub send_bye: bool,
    pub explicit_disconnect: bool,
}

#[derive(Debug)]
pub struct ReplacedSession {
    pub session_id: u64,
    pub endpoint: SocketAddr,
    pub shutdown_tx: Option<oneshot::Sender<SessionShutdown>>,
}

pub struct PeerManager {
    path: PathBuf,
    store: dashmap::DashMap<Uuid, PeerRecord>,
    live: dashmap::DashMap<Uuid, LivePeerSession>,
    // `RwLock` instead of `Mutex`: manual_targets is read-heavy (checked on
    // every reconnect cycle) and never held across an `.await` point.  Using
    // `std::sync::Mutex` in an async context risks blocking a Tokio worker
    // thread for the full duration of a lock contention window (HIGH-02).
    manual_targets: dashmap::DashMap<SocketAddr, u32>,
    next_session_id: AtomicU64,
}

impl PeerManager {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut store: PeerStoreData = if path.exists() {
            let bytes = std::fs::read(&path).context("reading peer store")?;
            if bytes.is_empty() {
                PeerStoreData::default()
            } else {
                serde_json::from_slice(&bytes).context("parsing peer store")?
            }
        } else {
            PeerStoreData::default()
        };

        // Connections do not persist across restarts.
        for peer in store.peers.values_mut() {
            peer.status = PeerConnectionState::Disconnected;
        }

        // One-time migration: a prior bug (see upsert_peer_ext) hard-coded
        // `remembered: true` on every bare mDNS/UDP sighting, so peer stores
        // written before this fix are full of phantom "known" devices that
        // were never actually paired. Correct that on load: `remembered`
        // should only be true if the peer has an actual relationship with
        // the user (trusted, or pairing is currently in flight) - the same
        // invariant `upsert_peer_ext` now maintains for newly-discovered
        // peers going forward.
        for peer in store.peers.values_mut() {
            if !peer.trusted && !peer.pairing_requested && !peer.outgoing_pairing_waiting {
                peer.remembered = false;
            }
        }

        let store_dashmap = dashmap::DashMap::new();
        for (k, v) in store.peers {
            store_dashmap.insert(k, v);
        }

        Ok(Self {
            path,
            store: store_dashmap,
            live: dashmap::DashMap::new(),
            manual_targets: dashmap::DashMap::new(),
            next_session_id: AtomicU64::new(1),
        })
    }

    pub fn save(&self) -> Result<()> {
        let path = self.path.clone();

        let store_to_save = PeerStoreData {
            peers: self
                .store
                .iter()
                .filter(|p| p.value().trusted || p.value().remembered)
                .map(|p| (*p.key(), p.value().clone()))
                .collect(),
        };

        let bytes = serde_json::to_vec_pretty(&store_to_save)?;

        let save_fn = move || {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            use rand::Rng;
            let rng_suffix: u32 = rand::thread_rng().gen();
            let tmp = path.with_extension(format!("tmp.{}", rng_suffix));
            if std::fs::write(&tmp, &bytes).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            } else {
                let _ = std::fs::remove_file(&tmp);
            }
        };

        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::spawn_blocking(save_fn);
        } else {
            save_fn();
        }
        Ok(())
    }

    pub fn list(&self) -> Vec<PeerRecord> {
        self.store.iter().map(|p| p.value().clone()).collect()
    }

    pub fn get(&self, device_id: Uuid) -> Option<PeerRecord> {
        self.store.get(&device_id).map(|p| p.value().clone())
    }

    pub fn upsert_peer(
        &self,
        device_id: Uuid,
        friendly_name: String,
        endpoint: SocketAddr,
        trusted: bool,
        discovery: DiscoverySource,
    ) -> Result<PeerRecord> {
        self.upsert_peer_ext(device_id, friendly_name, endpoint, trusted, discovery, None)
    }

    pub fn upsert_peer_ext(
        &self,
        device_id: Uuid,
        friendly_name: String,
        endpoint: SocketAddr,
        trusted: bool,
        discovery: DiscoverySource,
        platform: Option<String>,
    ) -> Result<PeerRecord> {
        if self.store.len() > 1000 {
            self.prune_stale_peers();
            if self.store.len() > 1000 {
                return Err(anyhow::anyhow!("Peer limit reached"));
            }
        }
        let now = now_secs();
        let record = {
            let is_placeholder = |name: &str| {
                name.starts_with("device-") || name.eq_ignore_ascii_case("Deskdrop Device")
            };

            // Deduplicate old peers if they have the exact same non-placeholder name
            // DO NOT deduplicate if the old peer is trusted to prevent unauthenticated
            // attackers from spoofing a name and deleting a trusted pairing (Unauthenticated DoS on Trust).
            if !is_placeholder(&friendly_name) {
                let mut duplicates = Vec::new();
                for p in self.store.iter() {
                    if *p.key() != device_id
                        && p.value().friendly_name == friendly_name
                        && p.value().status == PeerConnectionState::Disconnected
                        && !p.value().trusted
                    {
                        duplicates.push(*p.key());
                    }
                }
                for id in duplicates {
                    self.store.remove(&id);
                }
            }

            let mut record = self.store.entry(device_id).or_insert_with(|| PeerRecord {
                id: device_id,
                friendly_name: friendly_name.clone(),
                platform: platform.clone(),
                ips: vec![endpoint.ip()],
                port: endpoint.port(),
                trusted,
                // A bare discovery sighting (mDNS/UDP beacon) is not a
                // relationship with the user yet — only mark it "remembered"
                // (and thus shown as a known device, see PeerViewModel.IsKnown
                // on the Windows client) if it's already trusted. Otherwise it
                // surfaces as a transient "Nearby" entry until the user pairs.
                remembered: trusted,
                sync_enabled: true,
                remote_sync_enabled: true,
                auto_connect: true,
                last_seen: Some(now),
                last_sync: None,
                status: PeerConnectionState::Disconnected,
                discovery,
                last_error: None,
                explicit_disconnect: false,
                pairing_requested: false,
                outgoing_pairing_waiting: false,
                pairing_pin: None,
                last_discovery_at: Some(now),
                discovery_sources: vec![discovery],
                addr_history: vec![AddrRecord {
                    addr: endpoint,
                    last_seen_at: now,
                    success_count: 1,
                }],
                last_disconnect_at: None,
                lifecycle_state: None,
            });

            // Do not overwrite a real name with a placeholder name.
            if !is_placeholder(&friendly_name)
                || is_placeholder(&record.friendly_name)
                || record.friendly_name.is_empty()
            {
                record.friendly_name = friendly_name;
            }
            if platform.is_some() {
                record.platform = platform;
            }
            if !record.ips.contains(&endpoint.ip()) {
                record.ips.push(endpoint.ip());
            }
            record.port = endpoint.port();
            record.trusted = trusted;
            record.last_seen = Some(now);
            if record.discovery == DiscoverySource::Unknown {
                record.discovery = discovery;
            }

            // Track multi-layer discovery metadata.
            record.last_discovery_at = Some(now);
            if !record.discovery_sources.contains(&discovery) {
                record.discovery_sources.push(discovery);
            }
            // Maintain address history (cap at 10 entries).
            let addr = endpoint;
            if let Some(existing) = record.addr_history.iter_mut().find(|r| r.addr == addr) {
                existing.last_seen_at = now;
                existing.success_count = existing.success_count.saturating_add(1);
            } else {
                record.addr_history.push(AddrRecord {
                    addr,
                    last_seen_at: now,
                    success_count: 1,
                });
                // Keep only the 10 most recently seen.
                if record.addr_history.len() > 10 {
                    record
                        .addr_history
                        .sort_by_key(|b| std::cmp::Reverse(b.last_seen_at));
                    record.addr_history.truncate(10);
                }
            }

            record.clone()
        };

        self.save()?;
        Ok(record)
    }

    pub fn mark_connecting(&self, device_id: Uuid, endpoint: Option<SocketAddr>) -> Result<bool> {
        if let Some(endpoint) = endpoint {
            if self.live_endpoint(device_id) == Some(endpoint) {
                return Ok(false);
            }
        }

        {
            let mut entry = self.store.entry(device_id).or_insert_with(|| PeerRecord {
                id: device_id,
                port: endpoint
                    .map(|addr| addr.port())
                    .unwrap_or(crate::protocol::DEFAULT_PORT),
                ips: endpoint.map(|addr| vec![addr.ip()]).unwrap_or_default(),
                status: PeerConnectionState::Connecting,
                ..PeerRecord::default()
            });
            if entry.status == PeerConnectionState::Connecting
                || entry.status == PeerConnectionState::Connected
            {
                return Ok(false);
            }
            if let Some(endpoint) = endpoint {
                if !entry.ips.contains(&endpoint.ip()) {
                    entry.ips.push(endpoint.ip());
                }
                entry.port = endpoint.port();
            }
            entry.status = PeerConnectionState::Connecting;
            entry.last_error = None;
        }
        self.save()?;
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn replace_live_session(
        &self,
        local_device_id: Uuid,
        device_id: Uuid,
        is_outbound: bool,
        endpoint: SocketAddr,
        sender: mpsc::Sender<AppMessage>,
        file_sender: mpsc::Sender<AppMessage>,
        shutdown_tx: oneshot::Sender<SessionShutdown>,
    ) -> Result<(u64, Option<ReplacedSession>, bool)> {
        let we_are_initiator = local_device_id < device_id;
        let incoming_is_winner = we_are_initiator == is_outbound;

        if let Some(existing) = self.live.get(&device_id) {
            let existing_is_winner = we_are_initiator == existing.is_outbound;
            if !incoming_is_winner && existing_is_winner {
                return Ok((0, None, true)); // rejected_new = true
            }
        }

        let session_id = self.next_session_id.fetch_add(1, Ordering::Relaxed);
        {
            let mut entry = self.store.entry(device_id).or_insert_with(|| PeerRecord {
                id: device_id,
                port: endpoint.port(),
                ips: vec![endpoint.ip()],
                ..PeerRecord::default()
            });
            if !entry.ips.contains(&endpoint.ip()) {
                entry.ips.push(endpoint.ip());
            }
            entry.port = endpoint.port();
            entry.last_seen = Some(now_secs());
            entry.status = PeerConnectionState::Connected;
            entry.last_error = None;
        }

        let replaced = self.live.insert(
            device_id,
            LivePeerSession {
                session_id,
                endpoint,
                sender,
                file_sender,
                shutdown_tx: Some(shutdown_tx),
                is_outbound,
                connected_at: now_secs(),
            },
        );
        self.save()?;

        Ok((
            session_id,
            replaced.map(|session| ReplacedSession {
                session_id: session.session_id,
                endpoint: session.endpoint,
                shutdown_tx: session.shutdown_tx,
            }),
            false,
        ))
    }

    pub fn mark_disconnected(&self, device_id: Uuid, reason: Option<String>) -> Result<()> {
        self.live.remove(&device_id);
        {
            if let Some(mut entry) = self.store.get_mut(&device_id) {
                entry.status = PeerConnectionState::Disconnected;
                entry.last_error = reason;
                entry.last_disconnect_at = Some(now_secs());
            }
        }
        self.save()
    }

    pub fn mark_disconnected_if_current(
        &self,
        device_id: Uuid,
        session_id: u64,
        reason: Option<String>,
    ) -> Result<Option<u64>> {
        let connected_at = {
            if let Some(current) = self.live.get(&device_id) {
                if current.session_id != session_id {
                    return Ok(None);
                }
                current.connected_at
            } else {
                return Ok(None);
            }
        };

        self.mark_disconnected(device_id, reason)?;
        Ok(Some(connected_at))
    }

    pub fn mark_failed_all(&self, device_id: Uuid, reason: String) -> Result<()> {
        if self.live.contains_key(&device_id) {
            return Ok(());
        }
        self.live.remove(&device_id);
        {
            if let Some(mut entry) = self.store.get_mut(&device_id) {
                entry.status = PeerConnectionState::Failed;
                entry.last_error = Some(reason);
            }
        }
        self.save()
    }

    pub fn mark_failed(&self, device_id: Uuid, endpoint: SocketAddr, reason: String) -> Result<()> {
        if let Some(live_endpoint) = self.live_endpoint(device_id) {
            if live_endpoint != endpoint {
                {
                    if let Some(mut entry) = self.store.get_mut(&device_id) {
                        if !entry.ips.contains(&live_endpoint.ip()) {
                            entry.ips.push(live_endpoint.ip());
                        }
                        entry.port = live_endpoint.port();
                        entry.status = PeerConnectionState::Connected;
                        entry.last_error = Some(reason);
                    }
                }
                return self.save();
            }
        }

        self.live.remove(&device_id);
        {
            if let Some(mut entry) = self.store.get_mut(&device_id) {
                entry.status = PeerConnectionState::Failed;
                entry.last_error = Some(reason);
            }
        }
        self.save()
    }

    pub fn update_trust(&self, device_id: Uuid, trusted: bool) -> Result<()> {
        {
            if let Some(mut entry) = self.store.get_mut(&device_id) {
                entry.trusted = trusted;
            }
        }
        self.save()
    }

    pub fn update_last_sync(&self, device_id: Uuid) -> Result<()> {
        {
            if let Some(mut entry) = self.store.get_mut(&device_id) {
                entry.last_sync = Some(now_secs());
                entry.last_seen = Some(now_secs());
            }
        }
        self.save()
    }

    // ── Device lifecycle controls ─────────────────────────────────────────────

    pub fn set_sync_enabled(&self, device_id: Uuid, enabled: bool) -> Result<bool> {
        let found = {
            if let Some(mut entry) = self.store.get_mut(&device_id) {
                entry.sync_enabled = enabled;
                true
            } else {
                false
            }
        };
        if found {
            self.save()?;
        }
        Ok(found)
    }

    pub fn set_remote_sync_enabled(&self, device_id: Uuid, enabled: bool) -> Result<bool> {
        let found = {
            if let Some(mut entry) = self.store.get_mut(&device_id) {
                entry.remote_sync_enabled = enabled;
                true
            } else {
                false
            }
        };
        if found {
            self.save()?;
        }
        Ok(found)
    }

    pub fn set_auto_connect(&self, device_id: Uuid, auto_connect: bool) -> Result<bool> {
        let found = {
            if let Some(mut entry) = self.store.get_mut(&device_id) {
                entry.auto_connect = auto_connect;
                true
            } else {
                false
            }
        };
        if found {
            self.save()?;
        }
        Ok(found)
    }

    /// Sets whether this peer has an active pairing request pending.
    pub fn set_pairing_requested(&self, device_id: Uuid, requested: bool) -> Result<bool> {
        let changed = {
            if let Some(mut entry) = self.store.get_mut(&device_id) {
                entry.pairing_requested = requested;
                true
            } else {
                false
            }
        };
        if changed {
            self.save()?;
        }
        Ok(changed)
    }

    pub fn set_outgoing_pairing_waiting(&self, device_id: Uuid, waiting: bool) -> Result<bool> {
        let changed = {
            if let Some(mut entry) = self.store.get_mut(&device_id) {
                entry.outgoing_pairing_waiting = waiting;
                true
            } else {
                false
            }
        };
        if changed {
            self.save()?;
        }
        Ok(changed)
    }

    /// Sets the pairing PIN for this peer.
    pub fn set_pairing_pin(&self, device_id: Uuid, pin: Option<String>) -> Result<bool> {
        let found = {
            if let Some(mut entry) = self.store.get_mut(&device_id) {
                entry.pairing_pin = pin;
                true
            } else {
                false
            }
        };
        if found {
            self.save()?;
        }
        Ok(found)
    }

    pub fn set_explicit_disconnect(&self, device_id: Uuid, explicit: bool) -> Result<bool> {
        let found = {
            if let Some(mut entry) = self.store.get_mut(&device_id) {
                entry.explicit_disconnect = explicit;
                if explicit {
                    entry.status = PeerConnectionState::Disconnected;
                    entry.last_error = Some("manually disconnected".to_string());
                }
                true
            } else {
                false
            }
        };
        if found {
            self.save()?;
        }
        Ok(found)
    }

    pub fn is_explicitly_disconnected(&self, device_id: Uuid) -> bool {
        self.store
            .get(&device_id)
            .map(|entry| entry.explicit_disconnect)
            .unwrap_or(false)
    }

    pub fn forget_device(&self, device_id: Uuid) -> Result<bool> {
        let found = {
            if let Some(mut entry) = self.store.get_mut(&device_id) {
                entry.trusted = false;
                entry.remembered = false;
                entry.auto_connect = false;
                entry.explicit_disconnect = true;
                entry.pairing_requested = false;
                entry.pairing_pin = None;
                true
            } else {
                false
            }
        };
        if found {
            self.save()?;
        }
        Ok(found)
    }

    // ── Sender views ──────────────────────────────────────────────────────────

    /// Connected + trusted + sync_enabled peers — receives clipboard payloads.
    pub fn active_senders(&self) -> Vec<(Uuid, mpsc::Sender<AppMessage>)> {
        self.live
            .iter()
            .filter(|p| {
                self.store
                    .get(p.key())
                    .map(|p| p.value().is_sync_eligible())
                    .unwrap_or(false)
            })
            .map(|p| (*p.key(), p.value().sender.clone()))
            .collect()
    }

    pub fn sender(&self, device_id: Uuid) -> Option<mpsc::Sender<AppMessage>> {
        self.live.get(&device_id).map(|s| s.sender.clone())
    }

    pub fn file_sender(&self, device_id: Uuid) -> Option<mpsc::Sender<AppMessage>> {
        self.live.get(&device_id).map(|s| s.file_sender.clone())
    }

    /// All connected peers regardless of sync state (for heartbeats / control).
    pub fn all_connected_senders(&self) -> Vec<(Uuid, mpsc::Sender<AppMessage>)> {
        self.live
            .iter()
            .map(|p| (*p.key(), p.value().sender.clone()))
            .collect()
    }

    /// All connected AND trusted peers (for manual file transfers, ignoring sync_enabled).
    pub fn all_trusted_senders(&self) -> Vec<(Uuid, mpsc::Sender<AppMessage>)> {
        self.live
            .iter()
            .filter(|p| {
                self.store
                    .get(p.key())
                    .map(|r| r.value().trusted)
                    .unwrap_or(false)
            })
            .map(|p| (*p.key(), p.value().sender.clone()))
            .collect()
    }

    /// Returns trusted, remembered, disconnected peers that are eligible for
    /// active probing by the peer cache probe module.
    ///
    /// A peer is eligible if:
    /// - It is trusted and remembered
    /// - It is not currently connected
    /// - It has at least one known address (from addr_history or current IPs)
    /// - It was not explicitly disconnected by the user
    pub fn peers_needing_probe(&self) -> Vec<PeerRecord> {
        self.store
            .iter()
            .map(|p| p.value().clone())
            .filter(|p| {
                p.trusted
                    && p.remembered
                    && !p.explicit_disconnect
                    && !self.live.contains_key(&p.id)
                    && (p.status == PeerConnectionState::Disconnected
                        || p.status == PeerConnectionState::Failed)
                    && (!p.addr_history.is_empty() || !p.ips.is_empty())
            })
            .collect()
    }

    pub fn is_connected(&self, device_id: Uuid) -> bool {
        self.live.contains_key(&device_id)
    }

    pub fn live_endpoint(&self, device_id: Uuid) -> Option<SocketAddr> {
        self.live.get(&device_id).map(|s| s.endpoint)
    }

    pub fn endpoint_for(&self, device_id: Uuid) -> Option<SocketAddr> {
        self.get(device_id)
            .and_then(|record| record.socket_addrs().first().copied())
    }

    pub fn note_manual_target(&self, endpoint: SocketAddr) {
        self.manual_targets.entry(endpoint).or_insert(0);
    }

    pub fn record_manual_failure(&self, endpoint: SocketAddr) {
        *self.manual_targets.entry(endpoint).or_insert(0) += 1;
    }

    pub fn clear_manual_target(&self, endpoint: SocketAddr) {
        self.manual_targets.remove(&endpoint);
    }

    pub fn manual_targets(&self) -> Vec<SocketAddr> {
        self.manual_targets.iter().map(|r| *r.key()).collect()
    }

    pub fn shutdown_all_sessions(&self, reason: &str) -> Result<Vec<ReplacedSession>> {
        let keys: Vec<_> = self.live.iter().map(|x| *x.key()).collect();
        let mut sessions = Vec::new();
        for k in keys {
            if let Some(session) = self.live.remove(&k) {
                sessions.push(session);
            }
        }
        {
            for mut entry in self.store.iter_mut() {
                entry.status = PeerConnectionState::Disconnected;
                entry.last_error = Some(reason.to_string());
            }
        }
        self.save()?;
        Ok(sessions
            .into_iter()
            .map(|s| ReplacedSession {
                session_id: s.1.session_id,
                endpoint: s.1.endpoint,
                shutdown_tx: s.1.shutdown_tx,
            })
            .collect())
    }

    pub fn shutdown_peer_session(&self, device_id: Uuid) -> Result<Option<ReplacedSession>> {
        let removed = self.live.remove(&device_id);
        {
            if let Some(mut entry) = self.store.get_mut(&device_id) {
                entry.status = PeerConnectionState::Disconnected;
                entry.last_error = Some("manually disconnected".to_string());
            }
        }
        self.save()?;
        Ok(removed.map(|s| ReplacedSession {
            session_id: s.1.session_id,
            endpoint: s.1.endpoint,
            shutdown_tx: s.1.shutdown_tx,
        }))
    }

    pub fn last_sync_at(&self) -> Option<u64> {
        self.store
            .iter()
            .map(|p| p.value().clone())
            .filter_map(|p| p.last_sync)
            .max()
    }

    pub fn connected_count(&self) -> usize {
        self.live.len()
    }

    /// Prune in-memory peer records for devices that are:
    ///   1. Not currently connected (not in the live session map), AND
    ///   2. Not persisted across restarts (`remembered = false`), AND
    ///   3. Not trusted (pruning trusted-but-forgotten peers would lose their
    ///      TOFU key, requiring re-verification on next connect).
    ///
    /// The daemon is designed to run for months; without this, every
    /// transiently-seen device accumulates an entry in the peer store (MED-05).
    ///
    /// Call periodically (e.g. every 5 minutes from a background task).
    pub fn prune_stale_peers(&self) -> usize {
        let live_ids: std::collections::HashSet<Uuid> =
            self.live.iter().map(|r| *r.key()).collect();
        let now = now_secs();
        const STALE_THRESHOLD_SECS: u64 = 24 * 3600; // 24 hours
        let pruned = {
            let before = self.store.len();
            self.store.retain(|id, record| {
                // Always keep live connections
                if live_ids.contains(id) {
                    return true;
                }
                // Always keep trusted peers
                if record.trusted {
                    return true;
                }
                // Always keep remembered peers (see doc comment above).
                if record.remembered {
                    return true;
                }
                // Remove untrusted, unremembered + disconnected peers not seen in 24h
                let last_activity = record.last_seen.or(record.last_discovery_at).unwrap_or(0);
                if now.saturating_sub(last_activity) > STALE_THRESHOLD_SECS {
                    return false;
                }
                // Keep recent untrusted peers (they may be mid-pairing)
                true
            });
            before - self.store.len()
        };
        if pruned > 0 {
            tracing::info!(
                pruned,
                "pruned stale peer records (untrusted, not seen in 24h)"
            );
            let _ = self.save();
        }
        pruned
    }

    /// O(1) count of peers that are connected AND sync-eligible.
    ///
    /// Reads only the `live` session map (connected peers) and cross-checks
    /// `sync_enabled` from the persisted record, avoiding a full table scan.
    ///
    /// Uses `unwrap_or_else(|p| p.into_inner())` on both locks so a panicking
    /// task elsewhere cannot permanently poison the count (LOW-08).
    pub fn sync_eligible_count(&self) -> usize {
        let live_ids: Vec<Uuid> = self.live.iter().map(|r| *r.key()).collect();

        live_ids
            .iter()
            .filter(|id| {
                self.store
                    .get(id)
                    .map(|r| r.value().sync_enabled && r.value().trusted)
                    .unwrap_or(false)
            })
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use tempfile::NamedTempFile;
    use tokio::sync::oneshot;

    #[test]
    fn persists_peer_records() {
        let file = NamedTempFile::new().unwrap();
        let manager = PeerManager::load(file.path()).unwrap();
        let id = Uuid::new_v4();
        manager
            .upsert_peer(
                id,
                "Desk".into(),
                SocketAddr::from(([192, 168, 1, 8], 47823)),
                true,
                DiscoverySource::Manual,
            )
            .unwrap();
        let manager2 = PeerManager::load(file.path()).unwrap();
        let peers = manager2.list();
        assert_eq!(peers.len(), 1);
        assert!(peers[0].trusted);
    }

    #[test]
    fn pause_sync_suppresses_senders() {
        let file = NamedTempFile::new().unwrap();
        let manager = PeerManager::load(file.path()).unwrap();
        let id = Uuid::new_v4();
        manager
            .upsert_peer(
                id,
                "Phone".into(),
                SocketAddr::from(([192, 168, 1, 10], 47823)),
                true,
                DiscoverySource::Mdns,
            )
            .unwrap();
        let (tx, _rx) = mpsc::channel(1);
        let (stop, _stop_rx) = oneshot::channel();
        manager
            .replace_live_session(
                Uuid::nil(),
                id,
                true,
                SocketAddr::from(([192, 168, 1, 10], 47823)),
                tx.clone(),
                tx,
                stop,
            )
            .unwrap();
        assert_eq!(manager.active_senders().len(), 1);
        manager.set_sync_enabled(id, false).unwrap();
        assert_eq!(manager.active_senders().len(), 0);
        manager.set_sync_enabled(id, true).unwrap();
        assert_eq!(manager.active_senders().len(), 1);
    }

    #[test]
    fn untrusted_discovery_sighting_is_not_remembered() {
        let file = NamedTempFile::new().unwrap();
        let manager = PeerManager::load(file.path()).unwrap();
        let id = Uuid::new_v4();
        let peer = manager
            .upsert_peer(
                id,
                "device-abcd1234".into(),
                SocketAddr::from(([192, 168, 1, 30], 47823)),
                false,
                DiscoverySource::UdpBeacon,
            )
            .unwrap();
        assert!(
            !peer.remembered,
            "bare discovery sighting must not be treated as a known/paired device"
        );
        assert!(!peer.trusted);
    }

    #[test]
    fn trusted_upsert_is_remembered() {
        let file = NamedTempFile::new().unwrap();
        let manager = PeerManager::load(file.path()).unwrap();
        let id = Uuid::new_v4();
        let peer = manager
            .upsert_peer(
                id,
                "Laptop".into(),
                SocketAddr::from(([192, 168, 1, 31], 47823)),
                true,
                DiscoverySource::Mdns,
            )
            .unwrap();
        assert!(peer.remembered);
    }

    #[test]
    fn load_migrates_legacy_remembered_phantom_peers() {
        let file = NamedTempFile::new().unwrap();
        let id = Uuid::new_v4();
        {
            let manager = PeerManager::load(file.path()).unwrap();
            manager
                .upsert_peer(
                    id,
                    "device-abcd1234".into(),
                    SocketAddr::from(([192, 168, 1, 40], 47823)),
                    false,
                    DiscoverySource::UdpBeacon,
                )
                .unwrap();
            // Simulate a peer store written by the pre-fix code, which
            // hard-coded `remembered: true` on every bare discovery sighting.
            if let Some(mut entry) = manager.store.get_mut(&id) {
                entry.remembered = true;
            }
            manager.save().unwrap();
        }
        let reloaded = PeerManager::load(file.path()).unwrap();
        let peer = reloaded.get(id).unwrap();
        assert!(
            !peer.remembered,
            "legacy phantom peer must be demoted to Nearby on load"
        );
        assert!(!peer.trusted);
    }

    #[test]
    fn load_keeps_remembered_for_in_flight_pairing() {
        let file = NamedTempFile::new().unwrap();
        let id = Uuid::new_v4();
        {
            let manager = PeerManager::load(file.path()).unwrap();
            manager
                .upsert_peer(
                    id,
                    "Nearby Laptop".into(),
                    SocketAddr::from(([192, 168, 1, 41], 47823)),
                    false,
                    DiscoverySource::Mdns,
                )
                .unwrap();
            if let Some(mut entry) = manager.store.get_mut(&id) {
                entry.remembered = true;
                entry.pairing_requested = true;
            }
            manager.save().unwrap();
        }
        let reloaded = PeerManager::load(file.path()).unwrap();
        let peer = reloaded.get(id).unwrap();
        assert!(
            peer.remembered,
            "a peer with pairing genuinely in flight must not be demoted"
        );
    }

    #[test]
    fn prune_keeps_remembered_untrusted_peers_past_staleness_window() {
        let file = NamedTempFile::new().unwrap();
        let manager = PeerManager::load(file.path()).unwrap();
        let id = Uuid::new_v4();
        manager
            .upsert_peer(
                id,
                "Mid-pairing device".into(),
                SocketAddr::from(([192, 168, 1, 32], 47823)),
                false,
                DiscoverySource::Manual,
            )
            .unwrap();
        // Simulate a peer the user actually cares about (e.g. mid-pairing)
        // that just hasn't been seen recently.
        if let Some(mut entry) = manager.store.get_mut(&id) {
            entry.remembered = true;
            entry.last_seen = Some(0);
            entry.last_discovery_at = Some(0);
        }
        let pruned = manager.prune_stale_peers();
        assert_eq!(pruned, 0);
        assert!(manager.get(id).is_some());
    }

    #[test]
    fn forget_device_disables_auto_reconnect() {
        let file = NamedTempFile::new().unwrap();
        let manager = PeerManager::load(file.path()).unwrap();
        let id = Uuid::new_v4();
        manager
            .upsert_peer(
                id,
                "Tablet".into(),
                SocketAddr::from(([192, 168, 1, 20], 47823)),
                true,
                DiscoverySource::Mdns,
            )
            .unwrap();
        manager.forget_device(id).unwrap();
        let peer = manager.get(id).unwrap();
        assert!(!peer.trusted);
        assert!(!peer.remembered);
    }

    #[test]
    fn replacing_live_session_keeps_single_identity_record() {
        let file = NamedTempFile::new().unwrap();
        let manager = PeerManager::load(file.path()).unwrap();
        let id = Uuid::new_v4();
        manager
            .upsert_peer(
                id,
                "Desk".into(),
                SocketAddr::from(([192, 168, 1, 8], 47823)),
                true,
                DiscoverySource::Mdns,
            )
            .unwrap();
        let (tx1, _rx1) = mpsc::channel(1);
        let (stop1, _stop1_rx) = oneshot::channel();
        let (first_session_id, _, _) = manager
            .replace_live_session(
                Uuid::nil(),
                id,
                true,
                SocketAddr::from(([192, 168, 1, 8], 47823)),
                tx1.clone(),
                tx1,
                stop1,
            )
            .unwrap();
        let (tx2, _rx2) = mpsc::channel(1);
        let (stop2, _stop_rx2) = oneshot::channel();
        let (second_session_id, replaced, _) = manager
            .replace_live_session(
                Uuid::nil(),
                id,
                true,
                SocketAddr::from(([172, 20, 10, 4], 47823)),
                tx2.clone(),
                tx2,
                stop2,
            )
            .unwrap();
        let replaced = replaced.unwrap();
        assert_eq!(replaced.session_id, first_session_id);
        assert!(manager
            .mark_disconnected_if_current(id, first_session_id, Some("stale".into()))
            .unwrap()
            .is_none());
        assert!(manager
            .mark_disconnected_if_current(id, second_session_id, Some("closed".into()))
            .unwrap()
            .is_some());
        assert_eq!(manager.list().len(), 1);
        assert!(manager
            .get(id)
            .unwrap()
            .ips
            .contains(&IpAddr::V4(Ipv4Addr::new(172, 20, 10, 4))));
    }

    // ── Fix 14: connected_count and sync_eligible_count ───────────────────────

    #[test]
    fn connected_count_zero_when_no_sessions() {
        let file = NamedTempFile::new().unwrap();
        let manager = PeerManager::load(file.path()).unwrap();
        assert_eq!(manager.connected_count(), 0);
    }

    #[test]
    fn connected_count_increments_with_live_sessions() {
        let file = NamedTempFile::new().unwrap();
        let manager = PeerManager::load(file.path()).unwrap();

        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();

        for (id, name, ip) in [
            (id_a, "Alpha", [192, 168, 1, 10u8]),
            (id_b, "Beta", [192, 168, 1, 11]),
        ] {
            manager
                .upsert_peer(
                    id,
                    name.into(),
                    SocketAddr::from((ip, 47823)),
                    true,
                    DiscoverySource::Mdns,
                )
                .unwrap();
        }

        assert_eq!(manager.connected_count(), 0, "no live sessions yet");

        let (tx, _rx) = mpsc::channel(1);
        let (stop, _stop_rx) = oneshot::channel();
        manager
            .replace_live_session(
                Uuid::nil(),
                id_a,
                true,
                SocketAddr::from(([192, 168, 1, 10], 47823)),
                tx.clone(),
                tx,
                stop,
            )
            .unwrap();

        assert_eq!(manager.connected_count(), 1);
    }

    #[test]
    fn sync_eligible_count_excludes_untrusted_and_sync_disabled() {
        let file = NamedTempFile::new().unwrap();
        let manager = PeerManager::load(file.path()).unwrap();

        let id_trusted = Uuid::new_v4();
        let id_untrusted = Uuid::new_v4();
        let id_nosync = Uuid::new_v4();

        // Trusted + sync enabled.
        manager
            .upsert_peer(
                id_trusted,
                "Trusted".into(),
                SocketAddr::from(([10, 0, 0, 1], 47823)),
                true,
                DiscoverySource::Mdns,
            )
            .unwrap();
        // Untrusted.
        manager
            .upsert_peer(
                id_untrusted,
                "Stranger".into(),
                SocketAddr::from(([10, 0, 0, 2], 47823)),
                false,
                DiscoverySource::Mdns,
            )
            .unwrap();
        // Trusted but sync disabled.
        manager
            .upsert_peer(
                id_nosync,
                "NoSync".into(),
                SocketAddr::from(([10, 0, 0, 3], 47823)),
                true,
                DiscoverySource::Mdns,
            )
            .unwrap();
        manager.set_sync_enabled(id_nosync, false).unwrap();

        // Give all three a live session.
        for (id, ip) in [
            (id_trusted, [10, 0, 0, 1u8]),
            (id_untrusted, [10, 0, 0, 2]),
            (id_nosync, [10, 0, 0, 3]),
        ] {
            let (tx, _rx) = mpsc::channel(1);
            let (stop, _) = oneshot::channel();
            manager
                .replace_live_session(
                    Uuid::nil(),
                    id,
                    true,
                    SocketAddr::from((ip, 47823)),
                    tx.clone(),
                    tx,
                    stop,
                )
                .unwrap();
        }

        assert_eq!(manager.connected_count(), 3, "all three connected");
        // Only id_trusted passes both trusted AND sync_enabled.
        assert_eq!(manager.sync_eligible_count(), 1);
    }

    #[test]
    fn does_not_overwrite_real_name_with_placeholder() {
        let file = NamedTempFile::new().unwrap();
        let manager = PeerManager::load(file.path()).unwrap();
        let id = Uuid::new_v4();

        // 1. Initial real name.
        manager
            .upsert_peer(
                id,
                "My Macbook".into(),
                SocketAddr::from(([10, 0, 0, 1], 47823)),
                true,
                DiscoverySource::Mdns,
            )
            .unwrap();
        assert_eq!(manager.get(id).unwrap().friendly_name, "My Macbook");

        // 2. Try overwriting with "Deskdrop Device" (case insensitive).
        manager
            .upsert_peer(
                id,
                "Deskdrop Device".into(),
                SocketAddr::from(([10, 0, 0, 1], 47823)),
                true,
                DiscoverySource::Mdns,
            )
            .unwrap();
        assert_eq!(manager.get(id).unwrap().friendly_name, "My Macbook");

        manager
            .upsert_peer(
                id,
                "deskdrop device".into(),
                SocketAddr::from(([10, 0, 0, 1], 47823)),
                true,
                DiscoverySource::Mdns,
            )
            .unwrap();
        assert_eq!(manager.get(id).unwrap().friendly_name, "My Macbook");

        // 3. Try overwriting with "device-" prefix.
        manager
            .upsert_peer(
                id,
                "device-12345678".into(),
                SocketAddr::from(([10, 0, 0, 1], 47823)),
                true,
                DiscoverySource::Mdns,
            )
            .unwrap();
        assert_eq!(manager.get(id).unwrap().friendly_name, "My Macbook");

        // 4. Overwrite placeholder with real name.
        let id2 = Uuid::new_v4();
        manager
            .upsert_peer(
                id2,
                "Deskdrop Device".into(),
                SocketAddr::from(([10, 0, 0, 2], 47823)),
                true,
                DiscoverySource::Mdns,
            )
            .unwrap();
        assert_eq!(manager.get(id2).unwrap().friendly_name, "Deskdrop Device");

        manager
            .upsert_peer(
                id2,
                "My Macbook".into(),
                SocketAddr::from(([10, 0, 0, 2], 47823)),
                true,
                DiscoverySource::Mdns,
            )
            .unwrap();
        assert_eq!(manager.get(id2).unwrap().friendly_name, "My Macbook");
    }
}
