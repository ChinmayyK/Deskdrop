//! ClipRelay peer manager — device lifecycle + session registry.
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
use std::sync::{Mutex, RwLock};
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
    #[default]
    Unknown,
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
    pub ip: Option<IpAddr>,
    pub port: u16,

    // ── Lifecycle layers ──────────────────────────────────────────────────────
    pub trusted: bool,
    pub remembered: bool,
    pub sync_enabled: bool,
    pub auto_connect: bool,

    // ── Runtime state ─────────────────────────────────────────────────────────
    pub status: PeerConnectionState,
    pub last_seen: Option<u64>,
    pub last_sync: Option<u64>,
    pub discovery: DiscoverySource,
    pub last_error: Option<String>,
}

impl Default for PeerRecord {
    fn default() -> Self {
        Self {
            id: Uuid::nil(),
            friendly_name: String::new(),
            platform: None,
            ip: None,
            port: crate::protocol::DEFAULT_PORT,
            trusted: false,
            remembered: true,
            sync_enabled: true,
            auto_connect: true,
            status: PeerConnectionState::Disconnected,
            last_seen: None,
            last_sync: None,
            discovery: DiscoverySource::Unknown,
            last_error: None,
        }
    }
}

impl PeerRecord {
    pub fn socket_addr(&self) -> Option<SocketAddr> {
        self.ip.map(|ip| SocketAddr::new(ip, self.port))
    }

    /// Whether this peer should receive clipboard payloads right now.
    pub fn is_sync_eligible(&self) -> bool {
        self.trusted && self.sync_enabled
    }

    /// Whether this peer should reconnect automatically.
    pub fn should_auto_reconnect(&self) -> bool {
        self.trusted && self.remembered && self.auto_connect
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PeerStoreData {
    peers: HashMap<Uuid, PeerRecord>,
}

struct LivePeerSession {
    session_id: u64,
    endpoint: SocketAddr,
    sender: mpsc::Sender<AppMessage>,
    shutdown_tx: Option<oneshot::Sender<SessionShutdown>>,
}

#[derive(Debug)]
pub struct SessionShutdown {
    pub reason: String,
    pub send_bye: bool,
}

#[derive(Debug)]
pub struct ReplacedSession {
    pub session_id: u64,
    pub endpoint: SocketAddr,
    pub shutdown_tx: Option<oneshot::Sender<SessionShutdown>>,
}

pub struct PeerManager {
    path: PathBuf,
    store: RwLock<PeerStoreData>,
    live: RwLock<HashMap<Uuid, LivePeerSession>>,
    manual_targets: Mutex<HashMap<SocketAddr, u32>>,
    next_session_id: AtomicU64,
}

impl PeerManager {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let store = if path.exists() {
            let bytes = std::fs::read(&path).context("reading peer store")?;
            if bytes.is_empty() {
                PeerStoreData::default()
            } else {
                serde_json::from_slice(&bytes).context("parsing peer store")?
            }
        } else {
            PeerStoreData::default()
        };

        Ok(Self {
            path,
            store: RwLock::new(store),
            live: RwLock::new(HashMap::new()),
            manual_targets: Mutex::new(HashMap::new()),
            next_session_id: AtomicU64::new(1),
        })
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).context("creating peer dir")?;
        }
        let tmp = self.path.with_extension("tmp");
        let bytes = serde_json::to_vec_pretty(&*self.store.read().unwrap())?;
        std::fs::write(&tmp, bytes).context("writing peer store")?;
        std::fs::rename(&tmp, &self.path).context("renaming peer store")?;
        Ok(())
    }

    pub fn list(&self) -> Vec<PeerRecord> {
        self.store.read().unwrap().peers.values().cloned().collect()
    }

    pub fn get(&self, device_id: Uuid) -> Option<PeerRecord> {
        self.store.read().unwrap().peers.get(&device_id).cloned()
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
        let now = now_secs();
        let record = {
            let mut store = self.store.write().unwrap();
            let record = store.peers.entry(device_id).or_insert_with(|| PeerRecord {
                id: device_id,
                friendly_name: friendly_name.clone(),
                platform: platform.clone(),
                ip: Some(endpoint.ip()),
                port: endpoint.port(),
                trusted,
                remembered: true,
                sync_enabled: true,
                auto_connect: true,
                last_seen: Some(now),
                last_sync: None,
                status: PeerConnectionState::Disconnected,
                discovery,
                last_error: None,
            });

            record.friendly_name = friendly_name;
            if platform.is_some() {
                record.platform = platform;
            }
            record.ip = Some(endpoint.ip());
            record.port = endpoint.port();
            record.trusted = trusted;
            record.last_seen = Some(now);
            if record.discovery == DiscoverySource::Unknown {
                record.discovery = discovery;
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
            let mut store = self.store.write().unwrap();
            let entry = store.peers.entry(device_id).or_insert_with(|| PeerRecord {
                id: device_id,
                port: endpoint
                    .map(|addr| addr.port())
                    .unwrap_or(crate::protocol::DEFAULT_PORT),
                ip: endpoint.map(|addr| addr.ip()),
                status: PeerConnectionState::Connecting,
                ..PeerRecord::default()
            });
            if entry.status == PeerConnectionState::Connecting
                && endpoint.is_some()
                && entry.socket_addr() == endpoint
            {
                return Ok(false);
            }
            if let Some(endpoint) = endpoint {
                entry.ip = Some(endpoint.ip());
                entry.port = endpoint.port();
            }
            entry.status = PeerConnectionState::Connecting;
            entry.last_error = None;
        }
        self.save()?;
        Ok(true)
    }

    pub fn replace_live_session(
        &self,
        device_id: Uuid,
        endpoint: SocketAddr,
        sender: mpsc::Sender<AppMessage>,
        shutdown_tx: oneshot::Sender<SessionShutdown>,
    ) -> Result<(u64, Option<ReplacedSession>)> {
        let session_id = self.next_session_id.fetch_add(1, Ordering::Relaxed);
        {
            let mut store = self.store.write().unwrap();
            let entry = store.peers.entry(device_id).or_insert_with(|| PeerRecord {
                id: device_id,
                port: endpoint.port(),
                ip: Some(endpoint.ip()),
                ..PeerRecord::default()
            });
            entry.ip = Some(endpoint.ip());
            entry.port = endpoint.port();
            entry.last_seen = Some(now_secs());
            entry.status = PeerConnectionState::Connected;
            entry.last_error = None;
        }

        let replaced = self.live.write().unwrap().insert(
            device_id,
            LivePeerSession {
                session_id,
                endpoint,
                sender,
                shutdown_tx: Some(shutdown_tx),
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
        ))
    }

    pub fn mark_disconnected(&self, device_id: Uuid, reason: Option<String>) -> Result<()> {
        self.live.write().unwrap().remove(&device_id);
        {
            let mut store = self.store.write().unwrap();
            if let Some(entry) = store.peers.get_mut(&device_id) {
                entry.status = PeerConnectionState::Disconnected;
                entry.last_error = reason;
            }
        }
        self.save()
    }

    pub fn mark_disconnected_if_current(
        &self,
        device_id: Uuid,
        session_id: u64,
        reason: Option<String>,
    ) -> Result<bool> {
        {
            let live = self.live.read().unwrap();
            if let Some(current) = live.get(&device_id) {
                if current.session_id != session_id {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }

        self.mark_disconnected(device_id, reason)?;
        Ok(true)
    }

    pub fn mark_failed(&self, device_id: Uuid, endpoint: SocketAddr, reason: String) -> Result<()> {
        if let Some(live_endpoint) = self.live_endpoint(device_id) {
            if live_endpoint != endpoint {
                {
                    let mut store = self.store.write().unwrap();
                    if let Some(entry) = store.peers.get_mut(&device_id) {
                        entry.ip = Some(live_endpoint.ip());
                        entry.port = live_endpoint.port();
                        entry.status = PeerConnectionState::Connected;
                        entry.last_error = Some(reason);
                    }
                }
                return self.save();
            }
        }

        self.live.write().unwrap().remove(&device_id);
        {
            let mut store = self.store.write().unwrap();
            if let Some(entry) = store.peers.get_mut(&device_id) {
                entry.status = PeerConnectionState::Failed;
                entry.last_error = Some(reason);
            }
        }
        self.save()
    }

    pub fn update_trust(&self, device_id: Uuid, trusted: bool) -> Result<()> {
        {
            let mut store = self.store.write().unwrap();
            if let Some(entry) = store.peers.get_mut(&device_id) {
                entry.trusted = trusted;
            }
        }
        self.save()
    }

    pub fn update_last_sync(&self, device_id: Uuid) -> Result<()> {
        {
            let mut store = self.store.write().unwrap();
            if let Some(entry) = store.peers.get_mut(&device_id) {
                entry.last_sync = Some(now_secs());
                entry.last_seen = Some(now_secs());
            }
        }
        self.save()
    }

    // ── Device lifecycle controls ─────────────────────────────────────────────

    pub fn set_sync_enabled(&self, device_id: Uuid, enabled: bool) -> Result<bool> {
        let found = {
            let mut store = self.store.write().unwrap();
            if let Some(entry) = store.peers.get_mut(&device_id) {
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

    pub fn set_auto_connect(&self, device_id: Uuid, auto_connect: bool) -> Result<bool> {
        let found = {
            let mut store = self.store.write().unwrap();
            if let Some(entry) = store.peers.get_mut(&device_id) {
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

    /// Forget Device: removes persistent pairing without revoking trust.
    pub fn forget_device(&self, device_id: Uuid) -> Result<bool> {
        let found = {
            let mut store = self.store.write().unwrap();
            if let Some(entry) = store.peers.get_mut(&device_id) {
                entry.remembered = false;
                entry.auto_connect = false;
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
        let store = self.store.read().unwrap();
        self.live
            .read()
            .unwrap()
            .iter()
            .filter(|(id, _)| {
                store
                    .peers
                    .get(*id)
                    .map(|p| p.is_sync_eligible())
                    .unwrap_or(false)
            })
            .map(|(id, session)| (*id, session.sender.clone()))
            .collect()
    }

    /// All connected peers regardless of sync state (for heartbeats / control).
    pub fn all_connected_senders(&self) -> Vec<(Uuid, mpsc::Sender<AppMessage>)> {
        self.live
            .read()
            .unwrap()
            .iter()
            .map(|(id, session)| (*id, session.sender.clone()))
            .collect()
    }

    pub fn is_connected(&self, device_id: Uuid) -> bool {
        self.live.read().unwrap().contains_key(&device_id)
    }

    pub fn live_endpoint(&self, device_id: Uuid) -> Option<SocketAddr> {
        self.live
            .read()
            .unwrap()
            .get(&device_id)
            .map(|s| s.endpoint)
    }

    pub fn endpoint_for(&self, device_id: Uuid) -> Option<SocketAddr> {
        self.get(device_id).and_then(|record| record.socket_addr())
    }

    pub fn note_manual_target(&self, endpoint: SocketAddr) {
        self.manual_targets
            .lock()
            .unwrap()
            .entry(endpoint)
            .or_insert(0);
    }

    pub fn record_manual_failure(&self, endpoint: SocketAddr) {
        *self
            .manual_targets
            .lock()
            .unwrap()
            .entry(endpoint)
            .or_insert(0) += 1;
    }

    pub fn clear_manual_target(&self, endpoint: SocketAddr) {
        self.manual_targets.lock().unwrap().remove(&endpoint);
    }

    pub fn manual_targets(&self) -> Vec<SocketAddr> {
        self.manual_targets
            .lock()
            .unwrap()
            .keys()
            .copied()
            .collect()
    }

    pub fn shutdown_all_sessions(&self, reason: &str) -> Result<Vec<ReplacedSession>> {
        let sessions = {
            let mut live = self.live.write().unwrap();
            std::mem::take(&mut *live)
        };
        {
            let mut store = self.store.write().unwrap();
            for entry in store.peers.values_mut() {
                entry.status = PeerConnectionState::Disconnected;
                entry.last_error = Some(reason.to_string());
            }
        }
        self.save()?;
        Ok(sessions
            .into_values()
            .map(|s| ReplacedSession {
                session_id: s.session_id,
                endpoint: s.endpoint,
                shutdown_tx: s.shutdown_tx,
            })
            .collect())
    }

    pub fn shutdown_peer_session(&self, device_id: Uuid) -> Result<Option<ReplacedSession>> {
        let removed = self.live.write().unwrap().remove(&device_id);
        {
            let mut store = self.store.write().unwrap();
            if let Some(entry) = store.peers.get_mut(&device_id) {
                entry.status = PeerConnectionState::Disconnected;
                entry.last_error = Some("manually disconnected".to_string());
            }
        }
        self.save()?;
        Ok(removed.map(|s| ReplacedSession {
            session_id: s.session_id,
            endpoint: s.endpoint,
            shutdown_tx: s.shutdown_tx,
        }))
    }

    pub fn last_sync_at(&self) -> Option<u64> {
        self.store
            .read()
            .unwrap()
            .peers
            .values()
            .filter_map(|p| p.last_sync)
            .max()
    }

    pub fn connected_count(&self) -> usize {
        self.live.read().unwrap().len()
    }

    /// Fix 14: O(1) count of peers that are connected AND sync-eligible.
    ///
    /// Previously callers had to `list()` the entire peer store and filter,
    /// which is O(n) and called on every status-page refresh.  This helper
    /// reads only the `live` session map (connected peers) and cross-checks
    /// `sync_enabled` from the persisted record, avoiding a full table scan.
    pub fn sync_eligible_count(&self) -> usize {
        let live_ids: Vec<Uuid> = self.live.read().unwrap().keys().copied().collect();
        let store = self.store.read().unwrap();
        live_ids
            .iter()
            .filter(|id| {
                store
                    .peers
                    .get(*id)
                    .map(|r| r.sync_enabled && r.trusted)
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
            .replace_live_session(id, SocketAddr::from(([192, 168, 1, 10], 47823)), tx, stop)
            .unwrap();
        assert_eq!(manager.active_senders().len(), 1);
        manager.set_sync_enabled(id, false).unwrap();
        assert_eq!(manager.active_senders().len(), 0);
        manager.set_sync_enabled(id, true).unwrap();
        assert_eq!(manager.active_senders().len(), 1);
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
        let record = manager.get(id).unwrap();
        assert!(!record.remembered);
        assert!(!record.auto_connect);
        assert!(!record.should_auto_reconnect());
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
        let (first_session_id, _) = manager
            .replace_live_session(id, SocketAddr::from(([192, 168, 1, 8], 47823)), tx1, stop1)
            .unwrap();
        let (tx2, _rx2) = mpsc::channel(1);
        let (stop2, _stop2_rx) = oneshot::channel();
        let (second_session_id, replaced) = manager
            .replace_live_session(id, SocketAddr::from(([172, 20, 10, 4], 47823)), tx2, stop2)
            .unwrap();
        let replaced = replaced.unwrap();
        assert_eq!(replaced.session_id, first_session_id);
        assert!(!manager
            .mark_disconnected_if_current(id, first_session_id, Some("stale".into()))
            .unwrap());
        assert!(manager
            .mark_disconnected_if_current(id, second_session_id, Some("closed".into()))
            .unwrap());
        assert_eq!(manager.list().len(), 1);
        assert_eq!(
            manager.get(id).unwrap().ip,
            Some(IpAddr::V4(Ipv4Addr::new(172, 20, 10, 4)))
        );
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
            (id_b, "Beta",  [192, 168, 1, 11]),
        ] {
            manager
                .upsert_peer(id, name.into(), SocketAddr::from((ip, 47823)), true, DiscoverySource::Mdns)
                .unwrap();
        }

        assert_eq!(manager.connected_count(), 0, "no live sessions yet");

        let (tx, _rx) = mpsc::channel(1);
        let (stop, _) = oneshot::channel();
        manager
            .replace_live_session(id_a, SocketAddr::from(([192, 168, 1, 10], 47823)), tx, stop)
            .unwrap();

        assert_eq!(manager.connected_count(), 1);
    }

    #[test]
    fn sync_eligible_count_excludes_untrusted_and_sync_disabled() {
        let file = NamedTempFile::new().unwrap();
        let manager = PeerManager::load(file.path()).unwrap();

        let id_trusted   = Uuid::new_v4();
        let id_untrusted = Uuid::new_v4();
        let id_nosync    = Uuid::new_v4();

        // Trusted + sync enabled.
        manager
            .upsert_peer(id_trusted, "Trusted".into(), SocketAddr::from(([10, 0, 0, 1], 47823)), true, DiscoverySource::Mdns)
            .unwrap();
        // Untrusted.
        manager
            .upsert_peer(id_untrusted, "Stranger".into(), SocketAddr::from(([10, 0, 0, 2], 47823)), false, DiscoverySource::Mdns)
            .unwrap();
        // Trusted but sync disabled.
        manager
            .upsert_peer(id_nosync, "NoSync".into(), SocketAddr::from(([10, 0, 0, 3], 47823)), true, DiscoverySource::Mdns)
            .unwrap();
        manager.set_sync_enabled(id_nosync, false).unwrap();

        // Give all three a live session.
        for (id, ip) in [
            (id_trusted,   [10, 0, 0, 1u8]),
            (id_untrusted, [10, 0, 0, 2]),
            (id_nosync,    [10, 0, 0, 3]),
        ] {
            let (tx, _rx) = mpsc::channel(1);
            let (stop, _) = oneshot::channel();
            manager
                .replace_live_session(id, SocketAddr::from((ip, 47823)), tx, stop)
                .unwrap();
        }

        assert_eq!(manager.connected_count(), 3, "all three connected");
        // Only id_trusted passes both trusted AND sync_enabled.
        assert_eq!(manager.sync_eligible_count(), 1);
    }
}
