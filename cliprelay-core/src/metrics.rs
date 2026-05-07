//! ClipRelay runtime metrics.
//!
//! Provides a lightweight, lock-free-friendly snapshot of operational stats
//! that the UI layer can poll to show "Synced 12 items · avg 23 ms" status.
//!
//! All counters are `AtomicU64` so they can be updated from any thread
//! without locking.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use uuid::Uuid;

// ── Global counters ───────────────────────────────────────────────────────────

pub struct GlobalMetrics {
    /// Total clipboard pushes sent from this device.
    pub pushes_sent: AtomicU64,
    /// Total clipboard pushes received from all peers.
    pub pushes_received: AtomicU64,
    /// Total bytes sent (plaintext content).
    pub bytes_sent: AtomicU64,
    /// Total bytes received (plaintext content).
    pub bytes_received: AtomicU64,
    /// Total items suppressed by deduplication.
    pub dedup_suppressed: AtomicU64,
    /// Total items dropped by rate limiter.
    pub rate_limited: AtomicU64,
    /// Total connection errors.
    pub connection_errors: AtomicU64,
    /// Daemon start time.
    pub start_time: Instant,
}

impl GlobalMetrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            pushes_sent: AtomicU64::new(0),
            pushes_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            dedup_suppressed: AtomicU64::new(0),
            rate_limited: AtomicU64::new(0),
            connection_errors: AtomicU64::new(0),
            start_time: Instant::now(),
        })
    }

    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Human-readable summary line.
    pub fn summary(&self) -> String {
        format!(
            "↑{} pushes / {} KB  ↓{} pushes / {} KB  peers_ok  uptime {}s",
            self.pushes_sent.load(Relaxed),
            self.bytes_sent.load(Relaxed) / 1024,
            self.pushes_received.load(Relaxed),
            self.bytes_received.load(Relaxed) / 1024,
            self.uptime().as_secs(),
        )
    }
}

impl Default for GlobalMetrics {
    fn default() -> Self {
        Self {
            pushes_sent: AtomicU64::new(0),
            pushes_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            dedup_suppressed: AtomicU64::new(0),
            rate_limited: AtomicU64::new(0),
            connection_errors: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }
}

// ── Per-peer latency tracker ──────────────────────────────────────────────────

/// Rolling window of the last N round-trip times for a peer.
pub struct LatencyTracker {
    samples: Vec<u64>, // microseconds
    head: usize,
    full: bool,
    capacity: usize,
}

impl LatencyTracker {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: vec![0; capacity],
            head: 0,
            full: false,
            capacity,
        }
    }

    pub fn record(&mut self, rtt_us: u64) {
        self.samples[self.head] = rtt_us;
        self.head = (self.head + 1) % self.capacity;
        if self.head == 0 {
            self.full = true;
        }
    }

    fn active_samples(&self) -> &[u64] {
        if self.full {
            &self.samples
        } else {
            &self.samples[..self.head]
        }
    }

    pub fn avg_us(&self) -> Option<u64> {
        let s = self.active_samples();
        if s.is_empty() {
            return None;
        }
        Some(s.iter().sum::<u64>() / s.len() as u64)
    }

    pub fn min_us(&self) -> Option<u64> {
        self.active_samples().iter().copied().min()
    }

    pub fn max_us(&self) -> Option<u64> {
        self.active_samples().iter().copied().max()
    }

    pub fn p95_us(&self) -> Option<u64> {
        let mut s = self.active_samples().to_vec();
        if s.is_empty() {
            return None;
        }
        s.sort_unstable();
        let idx = (s.len() as f64 * 0.95) as usize;
        Some(s[idx.min(s.len() - 1)])
    }

    pub fn sample_count(&self) -> usize {
        self.active_samples().len()
    }
}

// ── Peer stats ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PeerStats {
    pub device_id: Uuid,
    pub device_name: String,
    pub connected_at: Instant,
    pub pushes_sent: u64,
    pub pushes_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub avg_rtt_us: Option<u64>,
    pub min_rtt_us: Option<u64>,
    pub max_rtt_us: Option<u64>,
    pub p95_rtt_us: Option<u64>,
}

impl PeerStats {
    pub fn session_duration(&self) -> Duration {
        self.connected_at.elapsed()
    }

    pub fn avg_rtt_ms(&self) -> Option<f64> {
        self.avg_rtt_us.map(|us| us as f64 / 1000.0)
    }
}

// ── Metrics registry ──────────────────────────────────────────────────────────

pub struct MetricsRegistry {
    pub global: Arc<GlobalMetrics>,
    peers: RwLock<HashMap<Uuid, PeerMetricsEntry>>,
}

struct PeerMetricsEntry {
    name: String,
    connected_at: Instant,
    pushes_sent: u64,
    pushes_received: u64,
    bytes_sent: u64,
    bytes_received: u64,
    latency: LatencyTracker,
}

impl MetricsRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            global: GlobalMetrics::new(),
            peers: RwLock::new(HashMap::new()),
        })
    }

    // ── Peer lifecycle ────────────────────────────────────────────────────────

    pub fn peer_connected(&self, device_id: Uuid, name: String) {
        self.peers.write().unwrap().insert(
            device_id,
            PeerMetricsEntry {
                name,
                connected_at: Instant::now(),
                pushes_sent: 0,
                pushes_received: 0,
                bytes_sent: 0,
                bytes_received: 0,
                latency: LatencyTracker::new(50),
            },
        );
    }

    pub fn peer_disconnected(&self, device_id: Uuid) {
        self.peers.write().unwrap().remove(&device_id);
    }

    // ── Record events ─────────────────────────────────────────────────────────

    pub fn record_send(&self, device_id: Uuid, bytes: u64) {
        self.global.pushes_sent.fetch_add(1, Relaxed);
        self.global.bytes_sent.fetch_add(bytes, Relaxed);
        if let Some(p) = self.peers.write().unwrap().get_mut(&device_id) {
            p.pushes_sent += 1;
            p.bytes_sent += bytes;
        }
    }

    pub fn record_receive(&self, device_id: Uuid, bytes: u64) {
        self.global.pushes_received.fetch_add(1, Relaxed);
        self.global.bytes_received.fetch_add(bytes, Relaxed);
        if let Some(p) = self.peers.write().unwrap().get_mut(&device_id) {
            p.pushes_received += 1;
            p.bytes_received += bytes;
        }
    }

    pub fn record_rtt(&self, device_id: Uuid, rtt_us: u64) {
        if let Some(p) = self.peers.write().unwrap().get_mut(&device_id) {
            p.latency.record(rtt_us);
        }
    }

    pub fn record_dedup_suppressed(&self) {
        self.global.dedup_suppressed.fetch_add(1, Relaxed);
    }

    pub fn record_rate_limited(&self) {
        self.global.rate_limited.fetch_add(1, Relaxed);
    }

    pub fn record_connection_error(&self) {
        self.global.connection_errors.fetch_add(1, Relaxed);
    }

    // ── Snapshots ─────────────────────────────────────────────────────────────

    pub fn all_peer_stats(&self) -> Vec<PeerStats> {
        self.peers
            .read()
            .unwrap()
            .iter()
            .map(|(id, p)| PeerStats {
                device_id: *id,
                device_name: p.name.clone(),
                connected_at: p.connected_at,
                pushes_sent: p.pushes_sent,
                pushes_received: p.pushes_received,
                bytes_sent: p.bytes_sent,
                bytes_received: p.bytes_received,
                avg_rtt_us: p.latency.avg_us(),
                min_rtt_us: p.latency.min_us(),
                max_rtt_us: p.latency.max_us(),
                p95_rtt_us: p.latency.p95_us(),
            })
            .collect()
    }

    pub fn peer_count(&self) -> usize {
        self.peers.read().unwrap().len()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latency_tracker_statistics() {
        let mut t = LatencyTracker::new(10);
        for &v in &[1000u64, 2000, 3000, 4000, 5000] {
            t.record(v);
        }
        assert_eq!(t.avg_us(), Some(3000));
        assert_eq!(t.min_us(), Some(1000));
        assert_eq!(t.max_us(), Some(5000));
        assert_eq!(t.sample_count(), 5);
    }

    #[test]
    fn registry_peer_lifecycle() {
        let reg = MetricsRegistry::new();
        let id = Uuid::new_v4();
        reg.peer_connected(id, "TestPeer".into());
        assert_eq!(reg.peer_count(), 1);
        reg.record_send(id, 1024);
        reg.record_rtt(id, 5000);
        let stats = reg.all_peer_stats();
        assert_eq!(stats[0].bytes_sent, 1024);
        assert_eq!(stats[0].avg_rtt_us, Some(5000));
        reg.peer_disconnected(id);
        assert_eq!(reg.peer_count(), 0);
    }
}
