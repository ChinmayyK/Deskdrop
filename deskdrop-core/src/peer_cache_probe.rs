//! Peer Cache Probe — active reachability probing for disconnected trusted peers.
//!
//! When a trusted peer disconnects, we don't want to rely solely on passive
//! discovery (mDNS / UDP beacons) to notice when it comes back online. This
//! module performs lightweight TCP connect-only probes against all known
//! historical addresses for a peer, using an adaptive back-off schedule:
//!
//! | Time since disconnect | Probe interval |
//! |-----------------------|----------------|
//! | 0 – 5 min             | 10 s           |
//! | 5 – 30 min            | 30 s           |
//! | > 30 min              | stop probing   |
//!
//! When a TCP connect succeeds, the module emits a `DiscoveredPeer` event via
//! the `DiscoveryInputHandle`, which flows through the unified discovery
//! manager into the engine's reconnect logic.
//!
//! The probe is connect-only: no handshake bytes are exchanged. We verify that
//! the port is accepting connections, then immediately close the socket. This
//! is intentionally cheap and avoids protocol version mismatches or auth
//! failures at the probe layer.

use crate::discovery_manager::{DiscoveredPeer, DiscoveryInputHandle};
use crate::peer_manager::DiscoverySource;
use crate::protocol::DEFAULT_PORT;

use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tracing::{debug, info, trace};
use uuid::Uuid;

// ── Constants ─────────────────────────────────────────────────────────────────

/// TCP connect timeout for probes. Kept short to avoid blocking the probe loop
/// on unreachable hosts sitting behind a firewall that drops SYN packets.
const PROBE_CONNECT_TIMEOUT: Duration = Duration::from_secs(1);

/// Tier 1: first 5 minutes after disconnect → probe every 10 seconds.
const TIER1_CUTOFF_SECS: u64 = 5 * 60;
const TIER1_INTERVAL: Duration = Duration::from_secs(10);

/// Tier 2: 5–30 minutes after disconnect → probe every 30 seconds.
const TIER2_CUTOFF_SECS: u64 = 30 * 60;
const TIER2_INTERVAL: Duration = Duration::from_secs(30);

/// After 30 minutes offline, stop probing entirely.
/// The peer is likely on a different network or powered off.

// ── Public types ──────────────────────────────────────────────────────────────

/// A peer to probe, with all known historical addresses and its disconnect timestamp.
#[derive(Debug, Clone)]
pub struct ProbeTarget {
    /// The peer's unique device identifier.
    pub device_id: Uuid,
    /// Human-readable device name (e.g. "Chinmay's Pixel 8").
    pub device_name: String,
    /// All known addresses (IP:port) for this peer, across all discovery sources.
    /// We try every address; any successful connect is enough.
    pub addrs: Vec<SocketAddr>,
    /// Unix timestamp (seconds) when this peer was last seen as disconnected.
    pub disconnected_at: u64,
}

/// Determine the appropriate probe interval based on how long a peer has been
/// disconnected. Returns `None` if probing should stop (peer offline > 30m).
///
/// # Arguments
///
/// * `seconds_since_disconnect` - elapsed seconds since the peer disconnected.
///
/// # Returns
///
/// * `Some(Duration)` - the interval to wait before the next probe.
/// * `None` - stop probing; the peer has been offline too long.
pub fn probe_interval(seconds_since_disconnect: u64) -> Option<Duration> {
    if seconds_since_disconnect < TIER1_CUTOFF_SECS {
        Some(TIER1_INTERVAL)
    } else if seconds_since_disconnect < TIER2_CUTOFF_SECS {
        Some(TIER2_INTERVAL)
    } else {
        None // Exceeded 30 minutes — stop probing.
    }
}

// ── PeerCacheProbe ────────────────────────────────────────────────────────────

/// Active probe controller that periodically checks whether disconnected
/// trusted peers are reachable again.
///
/// # Lifecycle
///
/// 1. The engine creates a `PeerCacheProbe` and spawns its `run()` method as a
///    tokio task.
/// 2. Periodically, the engine sends updated `Vec<ProbeTarget>` snapshots
///    through the `targets_rx` channel (e.g. whenever the peer list changes or
///    on a timer).
/// 3. The probe loop iterates over all targets, skips any that have exceeded
///    the 30-minute window, and performs TCP connect probes at the tier-appropriate
///    interval.
/// 4. On a successful probe, a `DiscoveredPeer` event is emitted via the
///    `DiscoveryInputHandle`, causing the discovery manager to re-announce the
///    peer to the engine.
pub struct PeerCacheProbe {
    /// Our own device ID (never probe ourselves).
    my_device_id: Uuid,
    /// Handle to emit discovered-peer events into the unified discovery pipeline.
    discovery_handle: DiscoveryInputHandle,
}

impl PeerCacheProbe {
    /// Create a new probe controller.
    ///
    /// # Arguments
    ///
    /// * `my_device_id` - this device's UUID (used to skip self-probing).
    /// * `discovery_handle` - handle into the discovery manager's input channel.
    pub fn new(my_device_id: Uuid, discovery_handle: DiscoveryInputHandle) -> Self {
        Self {
            my_device_id,
            discovery_handle,
        }
    }

    /// Main event loop. Receives target snapshots and probes disconnected peers.
    ///
    /// This method runs forever (until the `targets_rx` channel is closed or
    /// all senders are dropped). It should be spawned as a background tokio task.
    ///
    /// # Arguments
    ///
    /// * `targets_rx` - channel that delivers updated lists of peers to probe.
    ///   Each message replaces the previous target set.
    pub async fn run(self, mut targets_rx: mpsc::Receiver<Vec<ProbeTarget>>) {
        info!("peer cache probe: starting");

        let mut current_targets: Vec<ProbeTarget> = Vec::new();

        // The tick interval starts conservative and is dynamically adjusted
        // based on the shortest tier-interval across all active targets.
        let mut tick_interval = tokio::time::interval(TIER1_INTERVAL);
        tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // Accept updated target list from the engine.
                new_targets = targets_rx.recv() => {
                    match new_targets {
                        Some(targets) => {
                            debug!(
                                "peer cache probe: received {} targets",
                                targets.len()
                            );
                            current_targets = targets;
                            // Recalculate the tick interval based on the new target set.
                            if let Some(interval) = self.shortest_interval(&current_targets) {
                                tick_interval = tokio::time::interval(interval);
                                tick_interval.set_missed_tick_behavior(
                                    tokio::time::MissedTickBehavior::Skip,
                                );
                            }
                        }
                        None => {
                            info!("peer cache probe: target channel closed, shutting down");
                            break;
                        }
                    }
                }

                // Periodic probe tick.
                _ = tick_interval.tick() => {
                    self.probe_all(&mut current_targets).await;
                }
            }
        }

        info!("peer cache probe: stopped");
    }

    /// Probe all current targets, emitting discovery events for reachable peers.
    async fn probe_all(&self, targets: &mut Vec<ProbeTarget>) {
        if targets.is_empty() {
            return;
        }

        let now_secs = now_unix_secs();

        // Retain only targets that haven't exceeded the 30-minute window.
        targets.retain(|t| {
            let elapsed = now_secs.saturating_sub(t.disconnected_at);
            if probe_interval(elapsed).is_none() {
                debug!(
                    "peer cache probe: stopping probes for {} '{}' (offline {}s)",
                    t.device_id, t.device_name, elapsed
                );
                false
            } else {
                true
            }
        });

        for target in targets.iter() {
            // Never probe ourselves.
            if target.device_id == self.my_device_id {
                continue;
            }

            // Skip targets with no known addresses.
            if target.addrs.is_empty() {
                trace!(
                    "peer cache probe: skipping {} '{}' — no known addresses",
                    target.device_id,
                    target.device_name,
                );
                continue;
            }

            let elapsed = now_secs.saturating_sub(target.disconnected_at);
            let Some(_interval) = probe_interval(elapsed) else {
                continue;
            };

            trace!(
                "peer cache probe: probing {} '{}' ({} addrs, {}s since disconnect)",
                target.device_id,
                target.device_name,
                target.addrs.len(),
                elapsed,
            );

            if self.probe_peer(target).await {
                info!(
                    "peer cache probe: peer {} '{}' is reachable! Emitting discovery event.",
                    target.device_id, target.device_name
                );

                // Emit a DiscoveredPeer event through the discovery pipeline.
                // We use DiscoverySource::Unknown since this is a cache-based probe,
                // not a standard discovery mechanism. The discovery manager will merge
                // this with any existing records.
                let addrs = target
                    .addrs
                    .iter()
                    .map(|a| a.ip())
                    .collect::<Vec<_>>();
                let port = target
                    .addrs
                    .first()
                    .map(|a| a.port())
                    .unwrap_or(DEFAULT_PORT);

                let discovered = DiscoveredPeer {
                    device_id: target.device_id,
                    device_name: target.device_name.clone(),
                    addrs,
                    port,
                    source: DiscoverySource::Unknown,
                    protocol_version: None,
                    identity_fingerprint_prefix: None,
                };

                self.discovery_handle.found(discovered).await;
            }
        }
    }

    /// Attempt a TCP connect-only probe against all known addresses for a peer.
    ///
    /// Returns `true` if at least one address accepted a TCP connection.
    /// The connection is immediately dropped after a successful connect —
    /// no handshake or protocol data is exchanged.
    async fn probe_peer(&self, target: &ProbeTarget) -> bool {
        for addr in &target.addrs {
            match tokio::time::timeout(
                PROBE_CONNECT_TIMEOUT,
                TcpStream::connect(addr),
            )
            .await
            {
                Ok(Ok(_stream)) => {
                    // Connection succeeded — peer is reachable at this address.
                    // The stream is dropped immediately (close the socket).
                    debug!(
                        "peer cache probe: TCP connect to {} succeeded for {} '{}'",
                        addr, target.device_id, target.device_name
                    );
                    return true;
                }
                Ok(Err(err)) => {
                    // Connection refused or other I/O error — peer might be
                    // listening on a different port, or the address is stale.
                    trace!(
                        "peer cache probe: TCP connect to {} failed for {} '{}': {}",
                        addr, target.device_id, target.device_name, err
                    );
                }
                Err(_) => {
                    // Timeout — host is likely unreachable (firewall dropping SYN).
                    trace!(
                        "peer cache probe: TCP connect to {} timed out for {} '{}'",
                        addr, target.device_id, target.device_name
                    );
                }
            }
        }

        false
    }

    /// Compute the shortest probe interval across all active targets.
    /// Used to set the tick rate for the main loop.
    fn shortest_interval(&self, targets: &[ProbeTarget]) -> Option<Duration> {
        let now = now_unix_secs();
        targets
            .iter()
            .filter(|t| t.device_id != self.my_device_id)
            .filter_map(|t| {
                let elapsed = now.saturating_sub(t.disconnected_at);
                probe_interval(elapsed)
            })
            .min()
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Current unix timestamp in seconds.
fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddr};
    use tokio::net::TcpListener;

    // ── probe_interval tests ──────────────────────────────────────────────────

    #[test]
    fn tier1_interval_for_recent_disconnect() {
        // 0 seconds → tier 1 (30s).
        assert_eq!(probe_interval(0), Some(TIER1_INTERVAL));
        assert_eq!(probe_interval(0), Some(Duration::from_secs(10)));
    }

    #[test]
    fn tier1_interval_just_before_cutoff() {
        // 4 min 59s → still tier 1.
        let secs = TIER1_CUTOFF_SECS - 1;
        assert_eq!(probe_interval(secs), Some(TIER1_INTERVAL));
    }

    #[test]
    fn tier2_interval_at_cutoff() {
        // Exactly 5 minutes → tier 2 (2min).
        assert_eq!(probe_interval(TIER1_CUTOFF_SECS), Some(TIER2_INTERVAL));
        assert_eq!(
            probe_interval(TIER1_CUTOFF_SECS),
            Some(Duration::from_secs(30))
        );
    }

    #[test]
    fn tier2_interval_mid_range() {
        // 15 minutes → tier 2.
        assert_eq!(probe_interval(15 * 60), Some(TIER2_INTERVAL));
    }

    #[test]
    fn tier2_interval_just_before_cutoff() {
        // 29 min 59s → still tier 2.
        let secs = TIER2_CUTOFF_SECS - 1;
        assert_eq!(probe_interval(secs), Some(TIER2_INTERVAL));
    }

#[test]
    fn stops_probing_at_max() {
        // u64::MAX → stop.
        assert_eq!(probe_interval(u64::MAX), None);
    }

        #[test]
    fn stops_probing_at_30m() {
        assert_eq!(probe_interval(TIER2_CUTOFF_SECS), None);
        assert_eq!(probe_interval(TIER2_CUTOFF_SECS + 1), None);
    }

    // ── Interval boundary continuity ──────────────────────────────────────────

    #[test]
    fn intervals_are_monotonically_increasing_across_tiers() {
        let t1 = probe_interval(0).unwrap();
        let t2 = probe_interval(TIER1_CUTOFF_SECS).unwrap();

        assert!(t1 < t2, "tier1 < tier2");
    }

    #[test]
    fn all_intervals_are_positive() {
        for secs in [0, 60, 300, 600, 1800, 3600, 43200, 86399] {
            if let Some(interval) = probe_interval(secs) {
                assert!(interval > Duration::ZERO, "interval at {}s is zero", secs);
            }
        }
    }

    // ── ProbeTarget construction ──────────────────────────────────────────────

    #[test]
    fn probe_target_with_multiple_addrs() {
        let target = ProbeTarget {
            device_id: Uuid::new_v4(),
            device_name: "Test Device".into(),
            addrs: vec![
                SocketAddr::new(Ipv4Addr::new(192, 168, 1, 10).into(), DEFAULT_PORT),
                SocketAddr::new(Ipv4Addr::new(10, 0, 0, 5).into(), DEFAULT_PORT),
            ],
            disconnected_at: now_unix_secs(),
        };
        assert_eq!(target.addrs.len(), 2);
    }

    #[test]
    fn probe_target_with_empty_addrs() {
        let target = ProbeTarget {
            device_id: Uuid::new_v4(),
            device_name: "Empty".into(),
            addrs: vec![],
            disconnected_at: now_unix_secs(),
        };
        assert!(target.addrs.is_empty());
    }

    // ── Integration-style tests using a real TCP listener ─────────────────────

    #[tokio::test]
    async fn probe_peer_succeeds_against_real_listener() {
        // Bind a real TCP listener to detect a successful probe.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let bound_addr = listener.local_addr().unwrap();

        let (input_tx, _input_rx) = mpsc::channel(64);
        let handle = make_test_handle(input_tx);
        let probe = PeerCacheProbe::new(Uuid::new_v4(), handle);

        let target = ProbeTarget {
            device_id: Uuid::new_v4(),
            device_name: "Reachable Peer".into(),
            addrs: vec![bound_addr],
            disconnected_at: now_unix_secs(),
        };

        let result = probe.probe_peer(&target).await;
        assert!(result, "probe should succeed against a listening socket");

        // Clean up — drop listener.
        drop(listener);
    }

    #[tokio::test]
    async fn probe_peer_fails_against_closed_port() {
        // Bind and immediately close to get a port that is NOT listening.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let closed_addr = listener.local_addr().unwrap();
        drop(listener);

        let (input_tx, _input_rx) = mpsc::channel(64);
        let handle = make_test_handle(input_tx);
        let probe = PeerCacheProbe::new(Uuid::new_v4(), handle);

        let target = ProbeTarget {
            device_id: Uuid::new_v4(),
            device_name: "Unreachable Peer".into(),
            addrs: vec![closed_addr],
            disconnected_at: now_unix_secs(),
        };

        let result = probe.probe_peer(&target).await;
        assert!(!result, "probe should fail against a closed port");
    }

    #[tokio::test]
    async fn probe_peer_succeeds_on_second_address() {
        // First address is closed, second is listening.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let closed_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let good_addr = listener.local_addr().unwrap();
        let bad_addr = closed_listener.local_addr().unwrap();
        drop(closed_listener); // Close the first one.

        let (input_tx, _input_rx) = mpsc::channel(64);
        let handle = make_test_handle(input_tx);
        let probe = PeerCacheProbe::new(Uuid::new_v4(), handle);

        let target = ProbeTarget {
            device_id: Uuid::new_v4(),
            device_name: "Multi-Addr Peer".into(),
            addrs: vec![bad_addr, good_addr],
            disconnected_at: now_unix_secs(),
        };

        let result = probe.probe_peer(&target).await;
        assert!(
            result,
            "probe should succeed when at least one address is reachable"
        );

        drop(listener);
    }

    #[tokio::test]
    async fn probe_peer_fails_with_empty_addrs() {
        let (input_tx, _input_rx) = mpsc::channel(64);
        let handle = make_test_handle(input_tx);
        let probe = PeerCacheProbe::new(Uuid::new_v4(), handle);

        let target = ProbeTarget {
            device_id: Uuid::new_v4(),
            device_name: "No Addrs".into(),
            addrs: vec![],
            disconnected_at: now_unix_secs(),
        };

        let result = probe.probe_peer(&target).await;
        assert!(!result, "probe should fail with no addresses");
    }

    // ── probe_all tests ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn probe_all_emits_discovery_event_for_reachable_peer() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let bound_addr = listener.local_addr().unwrap();

        let (input_tx, mut input_rx) = mpsc::channel(64);
        let handle = make_test_handle(input_tx);
        let my_id = Uuid::new_v4();
        let probe = PeerCacheProbe::new(my_id, handle);

        let peer_id = Uuid::new_v4();
        let mut targets = vec![ProbeTarget {
            device_id: peer_id,
            device_name: "Reachable".into(),
            addrs: vec![bound_addr],
            disconnected_at: now_unix_secs(),
        }];

        probe.probe_all(&mut targets).await;

        // Should have emitted a discovery event.
        let event = tokio::time::timeout(Duration::from_millis(500), input_rx.recv())
            .await
            .expect("timeout waiting for discovery event")
            .expect("channel closed");

        match event {
            crate::discovery_manager::DiscoveryInput::Found(peer) => {
                assert_eq!(peer.device_id, peer_id);
                assert_eq!(peer.device_name, "Reachable");
                assert!(!peer.addrs.is_empty());
            }
            other => panic!("expected Found event, got {:?}", other),
        }

        drop(listener);
    }

    #[tokio::test]
    async fn probe_all_skips_self() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let bound_addr = listener.local_addr().unwrap();

        let (input_tx, mut input_rx) = mpsc::channel(64);
        let handle = make_test_handle(input_tx);
        let my_id = Uuid::new_v4();
        let probe = PeerCacheProbe::new(my_id, handle);

        // Target is our own device ID — should be skipped.
        let mut targets = vec![ProbeTarget {
            device_id: my_id,
            device_name: "Self".into(),
            addrs: vec![bound_addr],
            disconnected_at: now_unix_secs(),
        }];

        probe.probe_all(&mut targets).await;

        // Should NOT emit any event.
        let result = tokio::time::timeout(Duration::from_millis(100), input_rx.recv()).await;
        assert!(result.is_err(), "should not emit event for self");

        drop(listener);
    }

    #[tokio::test]
    async fn probe_all_evicts_expired_targets() {
        let (input_tx, _input_rx) = mpsc::channel(64);
        let handle = make_test_handle(input_tx);
        let probe = PeerCacheProbe::new(Uuid::new_v4(), handle);

        let now = now_unix_secs();
        let mut targets = vec![
            ProbeTarget {
                device_id: Uuid::new_v4(),
                device_name: "Recent".into(),
                addrs: vec![SocketAddr::new(
                    Ipv4Addr::new(192, 168, 1, 10).into(),
                    DEFAULT_PORT,
                )],
                disconnected_at: now, // Just now — should be retained.
            },
            ProbeTarget {
                device_id: Uuid::new_v4(),
                device_name: "Old".into(),
                addrs: vec![SocketAddr::new(
                    Ipv4Addr::new(192, 168, 1, 20).into(),
                    DEFAULT_PORT,
                )],
                disconnected_at: now - TIER2_CUTOFF_SECS, // >24h — should be evicted.
            },
        ];

        probe.probe_all(&mut targets).await;

        assert_eq!(targets.len(), 1, "expired target should have been evicted");
        assert_eq!(targets[0].device_name, "Recent");
    }

    // ── shortest_interval tests ───────────────────────────────────────────────

    #[test]
    fn shortest_interval_picks_minimum() {
        let my_id = Uuid::new_v4();
        let (input_tx, _) = mpsc::channel::<crate::discovery_manager::DiscoveryInput>(1);
        let handle = make_test_handle(input_tx);
        let probe = PeerCacheProbe::new(my_id, handle);

        let now = now_unix_secs();
        let targets = vec![
            ProbeTarget {
                device_id: Uuid::new_v4(),
                device_name: "Tier2".into(),
                addrs: vec![SocketAddr::new(
                    Ipv4Addr::new(10, 0, 0, 1).into(),
                    DEFAULT_PORT,
                )],
                disconnected_at: now - 600, // 10 min ago → tier 2.
            },
            ProbeTarget {
                device_id: Uuid::new_v4(),
                device_name: "Tier1".into(),
                addrs: vec![SocketAddr::new(
                    Ipv4Addr::new(10, 0, 0, 2).into(),
                    DEFAULT_PORT,
                )],
                disconnected_at: now, // Just now → tier 1 (30s).
            },
        ];

        let interval = probe.shortest_interval(&targets);
        assert_eq!(interval, Some(TIER1_INTERVAL));
    }

    #[test]
    fn shortest_interval_none_when_all_expired() {
        let my_id = Uuid::new_v4();
        let (input_tx, _) = mpsc::channel::<crate::discovery_manager::DiscoveryInput>(1);
        let handle = make_test_handle(input_tx);
        let probe = PeerCacheProbe::new(my_id, handle);

        let now = now_unix_secs();
        let targets = vec![ProbeTarget {
            device_id: Uuid::new_v4(),
            device_name: "Expired".into(),
            addrs: vec![SocketAddr::new(
                Ipv4Addr::new(10, 0, 0, 1).into(),
                DEFAULT_PORT,
            )],
            disconnected_at: now - TIER2_CUTOFF_SECS, // >24h.
        }];

        assert_eq!(probe.shortest_interval(&targets), None);
    }

    #[test]
    fn shortest_interval_none_when_empty() {
        let my_id = Uuid::new_v4();
        let (input_tx, _) = mpsc::channel::<crate::discovery_manager::DiscoveryInput>(1);
        let handle = make_test_handle(input_tx);
        let probe = PeerCacheProbe::new(my_id, handle);

        assert_eq!(probe.shortest_interval(&[]), None);
    }

    #[test]
    fn shortest_interval_skips_self() {
        let my_id = Uuid::new_v4();
        let (input_tx, _) = mpsc::channel::<crate::discovery_manager::DiscoveryInput>(1);
        let handle = make_test_handle(input_tx);
        let probe = PeerCacheProbe::new(my_id, handle);

        let now = now_unix_secs();
        let targets = vec![
            ProbeTarget {
                device_id: my_id, // Self — should be skipped.
                device_name: "Self".into(),
                addrs: vec![SocketAddr::new(
                    Ipv4Addr::new(127, 0, 0, 1).into(),
                    DEFAULT_PORT,
                )],
                disconnected_at: now,
            },
            ProbeTarget {
                device_id: Uuid::new_v4(),
                device_name: "Tier2".into(),
                addrs: vec![SocketAddr::new(
                    Ipv4Addr::new(10, 0, 0, 1).into(),
                    DEFAULT_PORT,
                )],
                disconnected_at: now - 600, // 10 min → tier 2.
            },
        ];

        let interval = probe.shortest_interval(&targets);
        assert_eq!(interval, Some(TIER2_INTERVAL));
    }

    // ── Full run loop test ────────────────────────────────────────────────────

    #[tokio::test]
    async fn run_loop_processes_targets_and_shuts_down() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let bound_addr = listener.local_addr().unwrap();

        let (input_tx, mut input_rx) = mpsc::channel(64);
        let handle = make_test_handle(input_tx);
        let my_id = Uuid::new_v4();
        let probe = PeerCacheProbe::new(my_id, handle);

        let (targets_tx, targets_rx) = mpsc::channel(8);
        let run_handle = tokio::spawn(probe.run(targets_rx));

        let peer_id = Uuid::new_v4();
        let targets = vec![ProbeTarget {
            device_id: peer_id,
            device_name: "RunLoopPeer".into(),
            addrs: vec![bound_addr],
            disconnected_at: now_unix_secs(),
        }];

        targets_tx.send(targets).await.unwrap();

        // Wait for the probe to fire and emit a discovery event.
        let event = tokio::time::timeout(Duration::from_secs(35), input_rx.recv())
            .await
            .expect("timeout waiting for probe event")
            .expect("channel closed");

        match event {
            crate::discovery_manager::DiscoveryInput::Found(peer) => {
                assert_eq!(peer.device_id, peer_id);
            }
            other => panic!("expected Found, got {:?}", other),
        }

        // Drop the sender to shut down the run loop.
        drop(targets_tx);
        let _ = tokio::time::timeout(Duration::from_secs(5), run_handle)
            .await
            .expect("run loop should terminate when targets channel closes");

        drop(listener);
    }

    // ── Test helpers ──────────────────────────────────────────────────────────

    /// Create a `DiscoveryInputHandle` from a raw `mpsc::Sender` for testing.
    /// This bypasses the normal `DiscoveryManager::new()` flow.
    fn make_test_handle(
        tx: mpsc::Sender<crate::discovery_manager::DiscoveryInput>,
    ) -> DiscoveryInputHandle {
        // SAFETY: DiscoveryInputHandle is a simple newtype around mpsc::Sender.
        // In tests we construct it directly. The struct has a single field `tx`.
        //
        // If DiscoveryInputHandle gains additional fields in the future, this
        // helper must be updated.
        //
        // This relies on DiscoveryInputHandle { tx } being constructible from
        // within the same crate.
        DiscoveryInputHandle { tx }
    }
}
