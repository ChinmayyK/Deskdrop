//! Hotspot discovery probe.
//!
//! When the local device is connected to a mobile hotspot (Android/iPhone),
//! standard discovery (mDNS, UDP broadcast) often fails because:
//! - Android hotspot restricts multicast/broadcast traffic
//! - mDNS responders may not be running on the hotspot host
//! - Network isolation on carrier-level hotspots
//!
//! This module actively probes the hotspot gateway to detect Deskdrop peers.
//! It works by:
//! 1. Detecting if we're on a hotspot subnet (192.168.43.x, 172.20.10.x, etc.)
//! 2. Identifying the gateway IP (usually x.x.x.1)
//! 3. Periodically attempting a TCP connect to the Deskdrop port on the gateway
//! 4. If successful, emitting a DiscoveredPeer event
//!
//! Crucially, this also works in the **reverse direction**: when the Android
//! device IS the hotspot host, it probes connected clients by scanning the
//! local subnet for Deskdrop peers.
//!
//! # Hotspot scenarios covered
//!
//! | Scenario                         | Detection              |
//! |----------------------------------|------------------------|
//! | Mac connected to Android hotspot | Gateway probe (.1)     |
//! | Mac connected to iPhone hotspot  | Gateway probe (.1)     |
//! | Android IS the hotspot host      | Subnet scan (.2-.20)   |
//! | USB tethering (Android→Mac)      | Gateway probe (.1)     |
//! | Wi-Fi Direct                     | Gateway probe (.1)     |

use crate::discovery_manager::{DiscoveredPeer, DiscoveryInputHandle};
use crate::network_manager::{self, NetworkInterfaceInfo};
use crate::peer_manager::DiscoverySource;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{debug, info, trace};
use uuid::Uuid;

/// How often to probe when on a hotspot network.
const HOTSPOT_PROBE_INTERVAL: Duration = Duration::from_secs(2);

/// TCP connect timeout for hotspot probes (hotspot latency is usually < 50ms).
const PROBE_TIMEOUT: Duration = Duration::from_millis(800);

/// Maximum number of IPs to scan on the hotspot subnet when we're the host.
const HOST_SCAN_MAX: u8 = 20;

/// Spawn the hotspot discovery probe.
///
/// This task runs continuously and:
/// - Monitors whether we're on a hotspot network
/// - When we are, probes the gateway and/or local subnet
/// - Emits DiscoveredPeer events for any responsive Deskdrop peers
///
/// The task automatically pauses when not on a hotspot and resumes when the
/// network changes to a hotspot subnet.
pub fn spawn_hotspot_probe(
    my_device_id: Uuid,
    port: u16,
    discovery_handle: DiscoveryInputHandle,
    mut network_rx: tokio::sync::watch::Receiver<Option<NetworkInterfaceInfo>>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(HOTSPOT_PROBE_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            // Get current network interface.
            let iface = match network_rx.borrow_and_update().clone() {
                Some(iface) => iface,
                None => continue,
            };

            if !network_manager::is_hotspot_network(&iface) {
                trace!("hotspot_probe: not on a hotspot network, skipping");
                continue;
            }

            debug!(
                "hotspot_probe: detected hotspot network on {} ({})",
                iface.name, iface.ip
            );

            // Determine if we're the hotspot HOST or a CLIENT.
            let is_host = is_likely_hotspot_host(&iface);

            if is_host {
                // We ARE the hotspot — scan connected clients.
                probe_hotspot_clients(
                    my_device_id,
                    &iface,
                    port,
                    &discovery_handle,
                )
                .await;
            } else {
                // We're a CLIENT — probe the gateway.
                probe_hotspot_gateway(
                    my_device_id,
                    &iface,
                    port,
                    &discovery_handle,
                )
                .await;
            }
        }
    });
}

/// Determine if this device is likely the hotspot host.
///
/// The host is typically at x.x.x.1 on the hotspot subnet.
fn is_likely_hotspot_host(iface: &NetworkInterfaceInfo) -> bool {
    match iface.ip {
        IpAddr::V4(ip) => {
            let o = ip.octets();
            // If our IP ends in .1, we're likely the gateway/host.
            o[3] == 1
        }
        _ => false,
    }
}

/// Probe the hotspot gateway for a Deskdrop peer.
///
/// Called when we're a CLIENT on a hotspot network.
async fn probe_hotspot_gateway(
    _my_device_id: Uuid,
    iface: &NetworkInterfaceInfo,
    port: u16,
    handle: &DiscoveryInputHandle,
) {
    let candidates = network_manager::detect_hotspot_gateway_candidates(iface);
    if candidates.is_empty() {
        debug!("hotspot_probe: no gateway candidates found");
        return;
    }

    for gateway_ip in candidates {
        let addr = SocketAddr::new(gateway_ip, port);
        if probe_tcp(addr).await {
            info!(
                "hotspot_probe: Deskdrop responding at gateway {} (hotspot host)",
                addr
            );
            handle
                .found(DiscoveredPeer {
                    device_id: gateway_device_id_placeholder(gateway_ip),
                    device_name: format!("Hotspot Host ({})", gateway_ip),
                    addrs: vec![gateway_ip],
                    port,
                    source: DiscoverySource::HotspotProbe,
                    protocol_version: None,
                    identity_fingerprint_prefix: None,
                })
                .await;
        }
    }
}

/// Scan the local hotspot subnet for Deskdrop clients.
///
/// Called when we ARE the hotspot host. Scans .2 through .20 (most hotspots
/// have very few clients).
async fn probe_hotspot_clients(
    _my_device_id: Uuid,
    iface: &NetworkInterfaceInfo,
    port: u16,
    handle: &DiscoveryInputHandle,
) {
    let base = match iface.ip {
        IpAddr::V4(ip) => ip,
        _ => return,
    };
    let o = base.octets();

    // Use a JoinSet to probe in parallel with bounded concurrency.
    let mut tasks = tokio::task::JoinSet::new();
    for i in 2..=HOST_SCAN_MAX {
        let ip = IpAddr::V4(Ipv4Addr::new(o[0], o[1], o[2], i));
        if ip == iface.ip {
            continue;
        }
        let addr = SocketAddr::new(ip, port);
        tasks.spawn(async move { (ip, addr, probe_tcp(addr).await) });
    }

    while let Some(result) = tasks.join_next().await {
        if let Ok((ip, addr, true)) = result {
            info!(
                "hotspot_probe: Deskdrop responding at client {} on hotspot subnet",
                addr
            );
            handle
                .found(DiscoveredPeer {
                    device_id: gateway_device_id_placeholder(ip),
                    device_name: format!("Hotspot Client ({})", ip),
                    addrs: vec![ip],
                    port,
                    source: DiscoverySource::HotspotProbe,
                    protocol_version: None,
                    identity_fingerprint_prefix: None,
                })
                .await;
        }
    }
}

/// Attempt a TCP connect to check if Deskdrop is listening at this address.
///
/// This does NOT perform a handshake — it only checks if the TCP port is open.
/// The full handshake happens later in `connect_once`.
async fn probe_tcp(addr: SocketAddr) -> bool {
    match timeout(PROBE_TIMEOUT, TcpStream::connect(addr)).await {
        Ok(Ok(_stream)) => {
            // Connection succeeded — Deskdrop is likely running.
            // The stream is dropped immediately (we don't send any data).
            true
        }
        Ok(Err(_)) => false,   // Connection refused or error
        Err(_) => false,       // Timeout
    }
}

/// Generate a placeholder device ID from the gateway IP.
///
/// Since we don't know the real device ID until the handshake, we generate
/// a deterministic UUID from the IP address. This will be replaced with the
/// real device ID after the ECDH handshake completes.
///
/// Using a deterministic UUID means repeated probes to the same IP won't
/// create duplicate peer entries in the discovery manager.
fn gateway_device_id_placeholder(ip: IpAddr) -> Uuid {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"deskdrop-hotspot-probe:");
    match ip {
        IpAddr::V4(v4) => hasher.update(v4.octets()),
        IpAddr::V6(v6) => hasher.update(v6.octets()),
    }
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    // Set UUID version 5 bits.
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotspot_host_detection() {
        let host = NetworkInterfaceInfo {
            name: "wlan0".into(),
            ip: IpAddr::V4(Ipv4Addr::new(192, 168, 43, 1)),
            is_primary: false,
        };
        assert!(is_likely_hotspot_host(&host));

        let client = NetworkInterfaceInfo {
            name: "en0".into(),
            ip: IpAddr::V4(Ipv4Addr::new(192, 168, 43, 100)),
            is_primary: false,
        };
        assert!(!is_likely_hotspot_host(&client));
    }

    #[test]
    fn placeholder_id_is_deterministic() {
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 43, 1));
        let id1 = gateway_device_id_placeholder(ip);
        let id2 = gateway_device_id_placeholder(ip);
        assert_eq!(id1, id2, "same IP should produce same placeholder UUID");

        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 43, 2));
        let id3 = gateway_device_id_placeholder(ip2);
        assert_ne!(id1, id3, "different IPs should produce different UUIDs");
    }

    #[test]
    fn hotspot_network_detection() {
        // Android hotspot client
        let android = NetworkInterfaceInfo {
            name: "en0".into(),
            ip: IpAddr::V4(Ipv4Addr::new(192, 168, 43, 5)),
            is_primary: false,
        };
        assert!(network_manager::is_hotspot_network(&android));

        // iPhone hotspot client
        let iphone = NetworkInterfaceInfo {
            name: "en0".into(),
            ip: IpAddr::V4(Ipv4Addr::new(172, 20, 10, 3)),
            is_primary: false,
        };
        assert!(network_manager::is_hotspot_network(&iphone));

        // Normal WiFi — NOT hotspot
        let normal = NetworkInterfaceInfo {
            name: "en0".into(),
            ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)),
            is_primary: false,
        };
        assert!(!network_manager::is_hotspot_network(&normal));

        // USB tethering
        let usb = NetworkInterfaceInfo {
            name: "en5".into(),
            ip: IpAddr::V4(Ipv4Addr::new(192, 168, 42, 2)),
            is_primary: false,
        };
        assert!(network_manager::is_hotspot_network(&usb));
    }

    #[tokio::test]
    async fn probe_tcp_returns_false_for_closed_port() {
        // Probe a port that's definitely not listening.
        let addr: SocketAddr = "127.0.0.1:1".parse().unwrap();
        assert!(!probe_tcp(addr).await);
    }
}
