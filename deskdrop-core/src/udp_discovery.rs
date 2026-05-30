//! UDP-based peer discovery — broadcast (Layer 2) and multicast (Layer 3).
//!
//! This module complements the mDNS-SD discovery layer with simpler, lower-latency
//! UDP beacon protocols that work on networks where mDNS is blocked, unreliable, or
//! unavailable (e.g. Android hotspot mode, corporate LANs with mDNS filtering).
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────────────┐
//! │                        UDP Discovery                                │
//! │                                                                      │
//! │  Layer 2: Broadcast (255.255.255.255:47824)                         │
//! │    • spawn_broadcast_beacon()   — sends periodic broadcast beacons  │
//! │    • spawn_broadcast_listener() — receives broadcast beacons        │
//! │                                                                      │
//! │  Layer 3: Multicast (239.255.77.77:47825)                           │
//! │    • spawn_multicast_beacon()   — sends periodic multicast beacons  │
//! │    • spawn_multicast_listener() — receives multicast beacons        │
//! │                                                                      │
//! │  Beacon format:                                                      │
//! │    DESKDROP3:<uuid>:<port>:<identity_fp_hex8>:<protocol_version>    │
//! │                                                                      │
//! │  All layers feed DiscoveredPeer events into the DiscoveryManager    │
//! │  via a shared DiscoveryInputHandle.                                  │
//! └──────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Beacon Payload
//!
//! The beacon is a compact UTF-8 string with fields separated by colons:
//!
//! | Field | Example | Description |
//! |-------|---------|-------------|
//! | Magic | `DESKDROP3` | Fixed magic prefix (includes major version) |
//! | UUID  | `550e8400-e29b-41d4-a716-446655440000` | Device ID |
//! | Port  | `47823` | TCP port for the clipboard sync protocol |
//! | FP    | `a1b2c3d4e5f6a7b8` | First 8 bytes of identity fingerprint (hex) |
//! | Ver   | `3` | Protocol version number |
//!
//! # Cross-platform notes
//!
//! - **macOS / Linux / Windows**: Full broadcast + multicast support.
//! - **Android**: Broadcast may be restricted on some OEMs; multicast works when
//!   the app holds a `WifiManager.MulticastLock`. The `socket2` crate handles
//!   platform-specific socket options.

use crate::discovery_manager::{DiscoveredPeer, DiscoveryInputHandle};
use crate::peer_manager::DiscoverySource;
use crate::protocol::PROTOCOL_VERSION;

use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, trace, warn};
use uuid::Uuid;

// ── Constants ─────────────────────────────────────────────────────────────────

/// UDP broadcast port for beacon advertisements.
pub const BROADCAST_PORT: u16 = 47824;

/// Multicast group address (IPv4, administratively scoped).
pub const MULTICAST_ADDR: &str = "239.255.77.77";

/// UDP multicast port for beacon advertisements.
pub const MULTICAST_PORT: u16 = 47825;

/// Magic prefix for beacon payloads. The trailing `3` denotes the wire format
/// generation — if the beacon layout ever changes incompatibly, bump this.
const BEACON_MAGIC: &str = "DESKDROP3";

/// Maximum beacon payload size. Beacons exceeding this are dropped.
/// A typical beacon is ~90 bytes; 512 bytes gives plenty of headroom.
const MAX_BEACON_SIZE: usize = 512;

/// Default beacon send interval.
const DEFAULT_BEACON_INTERVAL: Duration = Duration::from_secs(2);

/// Default receive buffer size.
const RECV_BUF_SIZE: usize = 1024;

// ── Configuration ─────────────────────────────────────────────────────────────

/// Configuration for UDP beacon discovery.
#[derive(Debug, Clone)]
pub struct UdpBeaconConfig {
    /// Interval between beacon transmissions.
    pub beacon_interval: Duration,
    /// The TCP port our daemon listens on (advertised in the beacon).
    pub service_port: u16,
    /// UDP port for broadcast beacons.
    pub broadcast_port: u16,
    /// UDP port for multicast beacons.
    pub multicast_port: u16,
    /// Multicast group address.
    pub multicast_addr: Ipv4Addr,
    /// Our device UUID.
    pub device_id: Uuid,
    /// First 8 bytes of our identity key fingerprint.
    pub identity_fingerprint_prefix: [u8; 8],
    /// Only accept peers with this protocol version. If `None`, accept any.
    pub required_protocol_version: Option<u16>,
}

impl Default for UdpBeaconConfig {
    fn default() -> Self {
        Self {
            beacon_interval: DEFAULT_BEACON_INTERVAL,
            service_port: crate::protocol::DEFAULT_PORT,
            broadcast_port: BROADCAST_PORT,
            multicast_port: MULTICAST_PORT,
            multicast_addr: MULTICAST_ADDR.parse().unwrap(),
            device_id: Uuid::nil(),
            identity_fingerprint_prefix: [0u8; 8],
            required_protocol_version: Some(PROTOCOL_VERSION),
        }
    }
}

// ── Parsed beacon data ────────────────────────────────────────────────────────

/// Parsed beacon payload.
#[derive(Debug, Clone, PartialEq, Eq)]
struct BeaconData {
    device_id: Uuid,
    port: u16,
    identity_fingerprint_prefix: [u8; 8],
    protocol_version: u16,
}

// ── Beacon formatting & parsing ───────────────────────────────────────────────

/// Format a beacon payload from the given parameters.
///
/// Layout: `DESKDROP3:<uuid>:<port>:<fp_hex16>:<version>`
fn format_beacon(
    device_id: Uuid,
    port: u16,
    identity_fp: &[u8; 8],
    protocol_version: u16,
) -> Vec<u8> {
    let fp_hex = hex::encode(identity_fp);
    format!(
        "{}:{}:{}:{}:{}",
        BEACON_MAGIC, device_id, port, fp_hex, protocol_version
    )
    .into_bytes()
}

/// Parse a beacon payload. Returns `None` if the payload is malformed.
fn parse_beacon(payload: &[u8]) -> Option<BeaconData> {
    // Beacons must be valid UTF-8.
    let text = std::str::from_utf8(payload).ok()?;
    let parts: Vec<&str> = text.split(':').collect();

    // Expected: DESKDROP3 : uuid (5 colon-separated sections) : port : fp_hex : version
    // UUID itself contains 4 hyphens but no colons, so `text.split(':')` yields:
    //   [0] = "DESKDROP3"
    //   [1] = uuid_str
    //   [2] = port_str
    //   [3] = fp_hex_str
    //   [4] = version_str
    if parts.len() != 5 {
        trace!(
            "beacon parse: expected 5 colon-separated fields, got {}",
            parts.len()
        );
        return None;
    }

    // Validate magic prefix.
    if parts[0] != BEACON_MAGIC {
        trace!("beacon parse: bad magic '{}', expected '{}'", parts[0], BEACON_MAGIC);
        return None;
    }

    // Parse UUID.
    let device_id = Uuid::parse_str(parts[1]).ok()?;

    // Parse port.
    let port: u16 = parts[2].parse().ok()?;

    // Parse fingerprint prefix (must be exactly 16 hex chars = 8 bytes).
    let fp_hex = parts[3];
    if fp_hex.len() != 16 {
        trace!(
            "beacon parse: fingerprint hex length {} != 16",
            fp_hex.len()
        );
        return None;
    }
    let fp_bytes = hex::decode(fp_hex).ok()?;
    let mut identity_fingerprint_prefix = [0u8; 8];
    identity_fingerprint_prefix.copy_from_slice(&fp_bytes);

    // Parse protocol version.
    let protocol_version: u16 = parts[4].parse().ok()?;

    Some(BeaconData {
        device_id,
        port,
        identity_fingerprint_prefix,
        protocol_version,
    })
}

/// Convert a parsed beacon + source address into a `DiscoveredPeer`.
fn beacon_to_peer(
    beacon: &BeaconData,
    source_addr: IpAddr,
    discovery_source: DiscoverySource,
) -> DiscoveredPeer {
    DiscoveredPeer {
        device_id: beacon.device_id,
        // Device name is not in the beacon — use a placeholder.
        // The real name is exchanged during the TCP handshake.
        device_name: format!("device-{}", &beacon.device_id.to_string()[..8]),
        addrs: vec![source_addr],
        port: beacon.port,
        source: discovery_source,
        protocol_version: Some(beacon.protocol_version),
        identity_fingerprint_prefix: Some(beacon.identity_fingerprint_prefix),
    }
}

/// Check if a beacon should be accepted based on config filters.
fn should_accept_beacon(
    beacon: &BeaconData,
    my_device_id: Uuid,
    required_version: Option<u16>,
) -> bool {
    // Skip our own beacons.
    if beacon.device_id == my_device_id {
        return false;
    }

    // Protocol version filter.
    if let Some(required) = required_version {
        if beacon.protocol_version != required {
            debug!(
                "udp: rejecting beacon from {} with protocol v{} (we require v{})",
                beacon.device_id, beacon.protocol_version, required
            );
            return false;
        }
    }

    true
}

// ── Broadcast beacon sender ───────────────────────────────────────────────────

/// Spawn a task that periodically sends UDP broadcast beacons.
///
/// The beacon is sent to `255.255.255.255:<broadcast_port>` (limited broadcast)
/// and also to subnet-directed broadcast addresses for all active IPv4
/// interfaces, improving delivery on networks that filter limited broadcast.
///
/// Returns a `CancellationToken` that can be used to stop the beacon.
pub async fn spawn_broadcast_beacon(
    config: UdpBeaconConfig,
    cancel: CancellationToken,
) -> Result<(), anyhow::Error> {
    let payload = format_beacon(
        config.device_id,
        config.service_port,
        &config.identity_fingerprint_prefix,
        PROTOCOL_VERSION,
    );

    let socket = create_broadcast_send_socket(config.broadcast_port)?;

    let broadcast_dest = SocketAddr::new(
        IpAddr::V4(Ipv4Addr::BROADCAST),
        config.broadcast_port,
    );

    info!(
        "udp broadcast beacon: starting (interval={:?}, port={}, device={})",
        config.beacon_interval,
        config.broadcast_port,
        &config.device_id.to_string()[..8]
    );

    // ── AirDrop-style startup burst ──────────────────────────────────────
    // Send 3 rapid beacons in the first 300ms so peers discover us
    // almost instantly, then fall back to the regular interval.
    for i in 0..3u8 {
        match socket.send_to(&payload, broadcast_dest).await {
            Ok(_) => trace!("udp broadcast beacon: startup burst {}/3 sent", i + 1),
            Err(e) => debug!("udp broadcast beacon: startup burst send failed: {}", e),
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let mut interval = tokio::time::interval(config.beacon_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("udp broadcast beacon: stopped");
                return Ok(());
            }
            _ = interval.tick() => {
                // Send to limited broadcast address.
                match socket.send_to(&payload, broadcast_dest).await {
                    Ok(n) => trace!("udp broadcast beacon: sent {} bytes to {}", n, broadcast_dest),
                    Err(e) => debug!("udp broadcast beacon: send to {} failed: {}", broadcast_dest, e),
                }

                // Also send to subnet-directed broadcast addresses for better delivery.
                if let Ok(ifaces) = if_addrs::get_if_addrs() {
                    for iface in ifaces {
                        if iface.is_loopback() {
                            continue;
                        }
                        if let Some(broadcast) = subnet_broadcast_addr(&iface) {
                            let dest = SocketAddr::new(IpAddr::V4(broadcast), config.broadcast_port);
                            if dest != broadcast_dest {
                                match socket.send_to(&payload, dest).await {
                                    Ok(_) => trace!("udp broadcast beacon: sent to subnet {}", dest),
                                    Err(e) => trace!("udp broadcast beacon: subnet {} send failed: {}", dest, e),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Broadcast beacon listener ─────────────────────────────────────────────────

/// Spawn a task that listens for UDP broadcast beacons and emits
/// `DiscoveredPeer` events via the provided `DiscoveryInputHandle`.
///
/// Returns when the `CancellationToken` is triggered.
pub async fn spawn_broadcast_listener(
    config: UdpBeaconConfig,
    input_handle: DiscoveryInputHandle,
    cancel: CancellationToken,
) -> Result<(), anyhow::Error> {
    let socket = create_broadcast_recv_socket(config.broadcast_port)?;

    info!(
        "udp broadcast listener: bound to 0.0.0.0:{}",
        config.broadcast_port
    );

    let mut buf = vec![0u8; RECV_BUF_SIZE];

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("udp broadcast listener: stopped");
                return Ok(());
            }
            result = socket.recv_from(&mut buf) => {
                match result {
                    Ok((n, src)) => {
                        if n > MAX_BEACON_SIZE {
                            trace!("udp broadcast: oversized packet ({} bytes) from {}", n, src);
                            continue;
                        }
                        handle_received_beacon(
                            &buf[..n],
                            src,
                            config.device_id,
                            config.required_protocol_version,
                            DiscoverySource::UdpBeacon,
                            &input_handle,
                        ).await;
                    }
                    Err(e) => {
                        warn!("udp broadcast listener: recv error: {}", e);
                        // Brief pause to avoid busy-looping on persistent errors.
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        }
    }
}

// ── Multicast beacon sender ──────────────────────────────────────────────────

/// Spawn a task that periodically sends UDP multicast beacons
/// to `239.255.77.77:<multicast_port>`.
pub async fn spawn_multicast_beacon(
    config: UdpBeaconConfig,
    cancel: CancellationToken,
) -> Result<(), anyhow::Error> {
    let payload = format_beacon(
        config.device_id,
        config.service_port,
        &config.identity_fingerprint_prefix,
        PROTOCOL_VERSION,
    );

    let multicast_dest = SocketAddr::new(
        IpAddr::V4(config.multicast_addr),
        config.multicast_port,
    );

    // Create a standard UDP socket for sending multicast.
    let socket = create_multicast_send_socket(config.multicast_addr)?;

    info!(
        "udp multicast beacon: starting (interval={:?}, group={}:{}, device={})",
        config.beacon_interval,
        config.multicast_addr,
        config.multicast_port,
        &config.device_id.to_string()[..8]
    );

    // ── AirDrop-style startup burst ──────────────────────────────────────
    for i in 0..3u8 {
        match socket.send_to(&payload, multicast_dest).await {
            Ok(_) => trace!("udp multicast beacon: startup burst {}/3 sent", i + 1),
            Err(e) => debug!("udp multicast beacon: startup burst send failed: {}", e),
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let mut interval = tokio::time::interval(config.beacon_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("udp multicast beacon: stopped");
                return Ok(());
            }
            _ = interval.tick() => {
                match socket.send_to(&payload, multicast_dest).await {
                    Ok(n) => trace!("udp multicast beacon: sent {} bytes to {}", n, multicast_dest),
                    Err(e) => debug!("udp multicast beacon: send failed: {}", e),
                }
            }
        }
    }
}

// ── Multicast beacon listener ─────────────────────────────────────────────────

/// Spawn a task that listens for UDP multicast beacons on `239.255.77.77:<multicast_port>`
/// and emits `DiscoveredPeer` events via the provided `DiscoveryInputHandle`.
pub async fn spawn_multicast_listener(
    config: UdpBeaconConfig,
    input_handle: DiscoveryInputHandle,
    cancel: CancellationToken,
) -> Result<(), anyhow::Error> {
    let multicast_addr = config.multicast_addr;
    let socket = create_multicast_recv_socket(multicast_addr, config.multicast_port)?;

    info!(
        "udp multicast listener: joined {}:{} on all interfaces",
        multicast_addr, config.multicast_port
    );

    let mut buf = vec![0u8; RECV_BUF_SIZE];

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("udp multicast listener: stopped");
                return Ok(());
            }
            result = socket.recv_from(&mut buf) => {
                match result {
                    Ok((n, src)) => {
                        if n > MAX_BEACON_SIZE {
                            trace!("udp multicast: oversized packet ({} bytes) from {}", n, src);
                            continue;
                        }
                        handle_received_beacon(
                            &buf[..n],
                            src,
                            config.device_id,
                            config.required_protocol_version,
                            DiscoverySource::UdpMulticast,
                            &input_handle,
                        ).await;
                    }
                    Err(e) => {
                        warn!("udp multicast listener: recv error: {}", e);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        }
    }
}

// ── Shared beacon handler ─────────────────────────────────────────────────────

/// Parse a received beacon payload and submit it as a `DiscoveredPeer` if valid.
async fn handle_received_beacon(
    payload: &[u8],
    src: SocketAddr,
    my_device_id: Uuid,
    required_version: Option<u16>,
    source: DiscoverySource,
    input_handle: &DiscoveryInputHandle,
) {
    let beacon = match parse_beacon(payload) {
        Some(b) => b,
        None => {
            trace!("udp {:?}: ignoring malformed beacon from {}", source, src);
            return;
        }
    };

    if !should_accept_beacon(&beacon, my_device_id, required_version) {
        return;
    }

    let peer = beacon_to_peer(&beacon, src.ip(), source);
    debug!(
        "udp {:?}: discovered peer {} at {}:{}",
        source, beacon.device_id, src.ip(), beacon.port
    );
    input_handle.found(peer).await;
}

// ── Socket creation helpers ───────────────────────────────────────────────────

/// Create a UDP socket suitable for sending broadcast packets.
fn create_broadcast_send_socket(_port: u16) -> Result<UdpSocket, anyhow::Error> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;

    // Allow sending to broadcast addresses.
    socket.set_broadcast(true)?;

    // Allow multiple sockets to bind to the same address (for collocated instances).
    socket.set_reuse_address(true)?;

    // macOS and BSDs support SO_REUSEPORT for load-balanced listeners.
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
    ))]
    socket.set_reuse_port(true)?;

    // Bind to any local address. Port 0 lets the OS pick an ephemeral port
    // since the sender doesn't need to listen on the broadcast port.
    let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
    socket.bind(&SockAddr::from(bind_addr))?;
    socket.set_nonblocking(true)?;

    let std_socket: std::net::UdpSocket = socket.into();
    Ok(UdpSocket::from_std(std_socket)?)
}

/// Create a UDP socket suitable for receiving broadcast packets.
fn create_broadcast_recv_socket(port: u16) -> Result<UdpSocket, anyhow::Error> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;

    socket.set_broadcast(true)?;
    socket.set_reuse_address(true)?;

    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
    ))]
    socket.set_reuse_port(true)?;

    // Bind to 0.0.0.0:<port> to receive broadcast packets.
    let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port);
    socket.bind(&SockAddr::from(bind_addr))?;
    socket.set_nonblocking(true)?;

    let std_socket: std::net::UdpSocket = socket.into();
    Ok(UdpSocket::from_std(std_socket)?)
}

/// Create a UDP socket suitable for sending multicast packets.
fn create_multicast_send_socket(
    multicast_addr: Ipv4Addr,
) -> Result<UdpSocket, anyhow::Error> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;

    socket.set_reuse_address(true)?;

    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
    ))]
    socket.set_reuse_port(true)?;

    // Set multicast TTL to 1 (link-local only) to prevent beacons from
    // leaking beyond the local network segment.
    socket.set_multicast_ttl_v4(1)?;

    // Enable multicast loopback so collocated instances on the same machine
    // can discover each other (useful during development).
    socket.set_multicast_loop_v4(true)?;

    // Set the outgoing multicast interface to all interfaces (0.0.0.0).
    socket.set_multicast_if_v4(&Ipv4Addr::UNSPECIFIED)?;

    let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
    socket.bind(&SockAddr::from(bind_addr))?;
    socket.set_nonblocking(true)?;

    // Suppress unused variable warning — multicast_addr is used to validate
    // the caller's intent but the actual group join happens on the receiver.
    let _ = multicast_addr;

    let std_socket: std::net::UdpSocket = socket.into();
    Ok(UdpSocket::from_std(std_socket)?)
}

/// Create a UDP socket that joins the multicast group for receiving beacons.
///
/// Uses `socket2` for the `join_multicast_v4` call which requires specifying
/// the interface address. We join on all non-loopback IPv4 interfaces to ensure
/// reception regardless of the system's routing table.
fn create_multicast_recv_socket(
    multicast_addr: Ipv4Addr,
    port: u16,
) -> Result<UdpSocket, anyhow::Error> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;

    socket.set_reuse_address(true)?;

    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
    ))]
    socket.set_reuse_port(true)?;

    // Bind to 0.0.0.0:<port> to receive multicast packets.
    let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port);
    socket.bind(&SockAddr::from(bind_addr))?;

    // Join the multicast group on all non-loopback IPv4 interfaces.
    let mut joined_any = false;
    match if_addrs::get_if_addrs() {
        Ok(ifaces) => {
            for iface in &ifaces {
                if iface.is_loopback() {
                    continue;
                }
                if let IpAddr::V4(ipv4) = iface.ip() {
                    match socket.join_multicast_v4(&multicast_addr, &ipv4) {
                        Ok(()) => {
                            debug!(
                                "udp multicast: joined {} on interface {} ({})",
                                multicast_addr, iface.name, ipv4
                            );
                            joined_any = true;
                        }
                        Err(e) => {
                            debug!(
                                "udp multicast: failed to join {} on {} ({}): {}",
                                multicast_addr, iface.name, ipv4, e
                            );
                        }
                    }
                }
            }
        }
        Err(e) => {
            warn!("udp multicast: failed to enumerate interfaces: {}", e);
        }
    }

    // Fallback: join on INADDR_ANY if no specific interface worked.
    if !joined_any {
        debug!("udp multicast: joining {} on INADDR_ANY (fallback)", multicast_addr);
        socket.join_multicast_v4(&multicast_addr, &Ipv4Addr::UNSPECIFIED)?;
    }

    socket.set_nonblocking(true)?;

    let std_socket: std::net::UdpSocket = socket.into();
    Ok(UdpSocket::from_std(std_socket)?)
}

// ── Interface helpers ─────────────────────────────────────────────────────────

/// Extract the subnet-directed broadcast address from a network interface, if available.
fn subnet_broadcast_addr(iface: &if_addrs::Interface) -> Option<Ipv4Addr> {
    match &iface.addr {
        if_addrs::IfAddr::V4(v4) => v4.broadcast,
        _ => None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Beacon formatting ─────────────────────────────────────────────────────

    #[test]
    fn format_beacon_produces_valid_utf8() {
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let fp = [0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6, 0xa7, 0xb8];
        let payload = format_beacon(id, 47823, &fp, 3);
        let text = std::str::from_utf8(&payload).expect("beacon should be valid UTF-8");
        assert!(text.starts_with("DESKDROP3:"));
    }

    #[test]
    fn format_beacon_contains_all_fields() {
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let fp = [0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6, 0xa7, 0xb8];
        let payload = format_beacon(id, 47823, &fp, 3);
        let text = std::str::from_utf8(&payload).unwrap();

        assert!(text.contains("550e8400-e29b-41d4-a716-446655440000"));
        assert!(text.contains("47823"));
        assert!(text.contains("a1b2c3d4e5f6a7b8"));
        assert!(text.ends_with(":3"));
    }

    #[test]
    fn format_beacon_field_count() {
        let id = Uuid::new_v4();
        let fp = [0u8; 8];
        let payload = format_beacon(id, 47823, &fp, 3);
        let text = std::str::from_utf8(&payload).unwrap();
        let fields: Vec<&str> = text.split(':').collect();
        assert_eq!(fields.len(), 5, "beacon should have exactly 5 colon-separated fields");
    }

    // ── Beacon parsing ────────────────────────────────────────────────────────

    #[test]
    fn parse_beacon_roundtrip() {
        let id = Uuid::new_v4();
        let fp = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
        let payload = format_beacon(id, 47823, &fp, 3);
        let parsed = parse_beacon(&payload).expect("should parse valid beacon");

        assert_eq!(parsed.device_id, id);
        assert_eq!(parsed.port, 47823);
        assert_eq!(parsed.identity_fingerprint_prefix, fp);
        assert_eq!(parsed.protocol_version, 3);
    }

    #[test]
    fn parse_beacon_rejects_empty() {
        assert!(parse_beacon(b"").is_none());
    }

    #[test]
    fn parse_beacon_rejects_garbage() {
        assert!(parse_beacon(b"this is not a beacon").is_none());
    }

    #[test]
    fn parse_beacon_rejects_wrong_magic() {
        let bad = b"DESKDROP2:550e8400-e29b-41d4-a716-446655440000:47823:a1b2c3d4e5f6a7b8:3";
        assert!(parse_beacon(bad).is_none());
    }

    #[test]
    fn parse_beacon_rejects_invalid_uuid() {
        let bad = b"DESKDROP3:not-a-uuid:47823:a1b2c3d4e5f6a7b8:3";
        assert!(parse_beacon(bad).is_none());
    }

    #[test]
    fn parse_beacon_rejects_invalid_port() {
        let bad = b"DESKDROP3:550e8400-e29b-41d4-a716-446655440000:99999:a1b2c3d4e5f6a7b8:3";
        assert!(parse_beacon(bad).is_none());
    }

    #[test]
    fn parse_beacon_rejects_short_fingerprint() {
        let bad = b"DESKDROP3:550e8400-e29b-41d4-a716-446655440000:47823:a1b2c3d4:3";
        assert!(parse_beacon(bad).is_none());
    }

    #[test]
    fn parse_beacon_rejects_long_fingerprint() {
        let bad = b"DESKDROP3:550e8400-e29b-41d4-a716-446655440000:47823:a1b2c3d4e5f6a7b8ff:3";
        assert!(parse_beacon(bad).is_none());
    }

    #[test]
    fn parse_beacon_rejects_non_hex_fingerprint() {
        let bad = b"DESKDROP3:550e8400-e29b-41d4-a716-446655440000:47823:zzzzzzzzzzzzzzzz:3";
        assert!(parse_beacon(bad).is_none());
    }

    #[test]
    fn parse_beacon_rejects_invalid_version() {
        let bad = b"DESKDROP3:550e8400-e29b-41d4-a716-446655440000:47823:a1b2c3d4e5f6a7b8:xyz";
        assert!(parse_beacon(bad).is_none());
    }

    #[test]
    fn parse_beacon_rejects_too_few_fields() {
        let bad = b"DESKDROP3:550e8400-e29b-41d4-a716-446655440000:47823";
        assert!(parse_beacon(bad).is_none());
    }

    #[test]
    fn parse_beacon_rejects_too_many_fields() {
        let bad = b"DESKDROP3:550e8400-e29b-41d4-a716-446655440000:47823:a1b2c3d4e5f6a7b8:3:extra";
        assert!(parse_beacon(bad).is_none());
    }

    #[test]
    fn parse_beacon_rejects_non_utf8() {
        let bad = vec![0xff, 0xfe, 0xfd, 0xfc];
        assert!(parse_beacon(&bad).is_none());
    }

    #[test]
    fn parse_beacon_zero_port_is_valid() {
        // Port 0 is technically valid in the format (though unlikely in production).
        let id = Uuid::new_v4();
        let fp = [0u8; 8];
        let payload = format_beacon(id, 0, &fp, 3);
        let parsed = parse_beacon(&payload).unwrap();
        assert_eq!(parsed.port, 0);
    }

    #[test]
    fn parse_beacon_max_port_is_valid() {
        let id = Uuid::new_v4();
        let fp = [0xff; 8];
        let payload = format_beacon(id, u16::MAX, &fp, PROTOCOL_VERSION);
        let parsed = parse_beacon(&payload).unwrap();
        assert_eq!(parsed.port, u16::MAX);
    }

    // ── Beacon filtering ──────────────────────────────────────────────────────

    #[test]
    fn should_accept_rejects_self_beacon() {
        let my_id = Uuid::new_v4();
        let beacon = BeaconData {
            device_id: my_id,
            port: 47823,
            identity_fingerprint_prefix: [0; 8],
            protocol_version: PROTOCOL_VERSION,
        };
        assert!(!should_accept_beacon(&beacon, my_id, Some(PROTOCOL_VERSION)));
    }

    #[test]
    fn should_accept_rejects_wrong_version() {
        let my_id = Uuid::new_v4();
        let peer_id = Uuid::new_v4();
        let beacon = BeaconData {
            device_id: peer_id,
            port: 47823,
            identity_fingerprint_prefix: [0; 8],
            protocol_version: 999,
        };
        assert!(!should_accept_beacon(&beacon, my_id, Some(PROTOCOL_VERSION)));
    }

    #[test]
    fn should_accept_allows_matching_version() {
        let my_id = Uuid::new_v4();
        let peer_id = Uuid::new_v4();
        let beacon = BeaconData {
            device_id: peer_id,
            port: 47823,
            identity_fingerprint_prefix: [0; 8],
            protocol_version: PROTOCOL_VERSION,
        };
        assert!(should_accept_beacon(&beacon, my_id, Some(PROTOCOL_VERSION)));
    }

    #[test]
    fn should_accept_allows_any_version_when_filter_is_none() {
        let my_id = Uuid::new_v4();
        let peer_id = Uuid::new_v4();
        let beacon = BeaconData {
            device_id: peer_id,
            port: 47823,
            identity_fingerprint_prefix: [0; 8],
            protocol_version: 999,
        };
        assert!(should_accept_beacon(&beacon, my_id, None));
    }

    // ── Beacon to peer conversion ─────────────────────────────────────────────

    #[test]
    fn beacon_to_peer_sets_correct_fields() {
        let beacon = BeaconData {
            device_id: Uuid::new_v4(),
            port: 47823,
            identity_fingerprint_prefix: [0xaa; 8],
            protocol_version: PROTOCOL_VERSION,
        };
        let src_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 42));
        let peer = beacon_to_peer(&beacon, src_ip, DiscoverySource::UdpBeacon);

        assert_eq!(peer.device_id, beacon.device_id);
        assert_eq!(peer.addrs, vec![src_ip]);
        assert_eq!(peer.port, 47823);
        assert_eq!(peer.source, DiscoverySource::UdpBeacon);
        assert_eq!(peer.protocol_version, Some(PROTOCOL_VERSION));
        assert_eq!(peer.identity_fingerprint_prefix, Some([0xaa; 8]));
    }

    #[test]
    fn beacon_to_peer_multicast_source() {
        let beacon = BeaconData {
            device_id: Uuid::new_v4(),
            port: 12345,
            identity_fingerprint_prefix: [0; 8],
            protocol_version: PROTOCOL_VERSION,
        };
        let src_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5));
        let peer = beacon_to_peer(&beacon, src_ip, DiscoverySource::UdpMulticast);

        assert_eq!(peer.source, DiscoverySource::UdpMulticast);
    }

    #[test]
    fn beacon_to_peer_device_name_is_placeholder() {
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let beacon = BeaconData {
            device_id: id,
            port: 47823,
            identity_fingerprint_prefix: [0; 8],
            protocol_version: PROTOCOL_VERSION,
        };
        let peer = beacon_to_peer(
            &beacon,
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            DiscoverySource::UdpBeacon,
        );
        assert_eq!(peer.device_name, "device-550e8400");
    }

    // ── Config defaults ───────────────────────────────────────────────────────

    #[test]
    fn config_defaults_are_sane() {
        let config = UdpBeaconConfig::default();
        assert_eq!(config.broadcast_port, BROADCAST_PORT);
        assert_eq!(config.multicast_port, MULTICAST_PORT);
        assert_eq!(config.multicast_addr, MULTICAST_ADDR.parse::<Ipv4Addr>().unwrap());
        assert_eq!(config.service_port, crate::protocol::DEFAULT_PORT);
        assert_eq!(config.required_protocol_version, Some(PROTOCOL_VERSION));
        assert!(config.beacon_interval >= Duration::from_secs(1));
    }

    // ── Constants ─────────────────────────────────────────────────────────────

    #[test]
    fn constants_have_expected_values() {
        assert_eq!(BROADCAST_PORT, 47824);
        assert_eq!(MULTICAST_PORT, 47825);
        assert_eq!(MULTICAST_ADDR, "239.255.77.77");
    }

    #[test]
    fn multicast_addr_is_in_admin_scoped_range() {
        let addr: Ipv4Addr = MULTICAST_ADDR.parse().unwrap();
        // 239.0.0.0/8 is administratively scoped (RFC 2365).
        assert_eq!(addr.octets()[0], 239);
    }

    // ── Roundtrip with various UUIDs ──────────────────────────────────────────

    #[test]
    fn roundtrip_with_nil_uuid() {
        let id = Uuid::nil();
        let fp = [0u8; 8];
        let payload = format_beacon(id, 47823, &fp, 3);
        let parsed = parse_beacon(&payload).unwrap();
        assert_eq!(parsed.device_id, id);
    }

    #[test]
    fn roundtrip_with_max_uuid() {
        let id = Uuid::max();
        let fp = [0xff; 8];
        let payload = format_beacon(id, 47823, &fp, PROTOCOL_VERSION);
        let parsed = parse_beacon(&payload).unwrap();
        assert_eq!(parsed.device_id, id);
        assert_eq!(parsed.identity_fingerprint_prefix, [0xff; 8]);
    }

    #[test]
    fn roundtrip_preserves_all_fingerprint_bytes() {
        let id = Uuid::new_v4();
        let fp = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
        let payload = format_beacon(id, 1234, &fp, PROTOCOL_VERSION);
        let parsed = parse_beacon(&payload).unwrap();
        assert_eq!(parsed.identity_fingerprint_prefix, fp);
    }

    // ── Stress: many random roundtrips ────────────────────────────────────────

    #[test]
    fn roundtrip_many_random_beacons() {
        for _ in 0..100 {
            let id = Uuid::new_v4();
            let port = rand::random::<u16>();
            let fp: [u8; 8] = rand::random();
            let version = rand::random::<u16>() % 100;
            let payload = format_beacon(id, port, &fp, version);
            let parsed = parse_beacon(&payload).expect("roundtrip should always succeed");
            assert_eq!(parsed.device_id, id);
            assert_eq!(parsed.port, port);
            assert_eq!(parsed.identity_fingerprint_prefix, fp);
            assert_eq!(parsed.protocol_version, version);
        }
    }

    // ── Beacon size ───────────────────────────────────────────────────────────

    #[test]
    fn beacon_fits_within_max_size() {
        let id = Uuid::max(); // Longest possible UUID representation.
        let fp = [0xff; 8]; // Longest hex encoding.
        let payload = format_beacon(id, u16::MAX, &fp, u16::MAX);
        assert!(
            payload.len() <= MAX_BEACON_SIZE,
            "beacon size {} exceeds MAX_BEACON_SIZE {}",
            payload.len(),
            MAX_BEACON_SIZE
        );
    }

    #[test]
    fn beacon_size_is_reasonable() {
        let id = Uuid::new_v4();
        let fp = [0xab; 8];
        let payload = format_beacon(id, 47823, &fp, 3);
        // A typical beacon should be around 75-95 bytes.
        assert!(payload.len() < 150, "beacon is unexpectedly large: {} bytes", payload.len());
        assert!(payload.len() > 50, "beacon is unexpectedly small: {} bytes", payload.len());
    }
}
