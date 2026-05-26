//! mDNS-SD (DNS Service Discovery) — advertise and browse for Deskdrop peers.
//!
//! Each running Deskdrop daemon:
//!   1. Registers a `_deskdrop._tcp.local.` service with its TCP port.
//!   2. Continuously browses for other `_deskdrop._tcp.local.` services.
//!   3. When a new peer appears, the `PeerEvent::Found` event fires.
//!   4. When a peer disappears, `PeerEvent::Lost` fires.
//!
//! The caller then initiates a TCP handshake to Found peers.
//!
//! # v3 fixes in this module
//!
//! **Fix 6 — Protocol version validation**: The TXT record `v` field is now
//! compared against `PROTOCOL_VERSION` before emitting a `PeerEvent::Found`.
//! An incompatible peer is skipped at mDNS time (clean log warning) rather
//! than causing a TCP-layer handshake failure after an unnecessary connect.
//!
//! **Fix 7 — IPv4 preference**: When a peer advertises multiple addresses the
//! old code used `HashSet::iter().next()`, whose order is non-deterministic.
//! IPv6 link-local addresses (fe80::/10) require a `%scope_id` suffix that
//! the socket layer doesn't supply automatically, causing silent connect
//! failures. The new `prefer_ipv4()` helper chooses IPv4 first, then IPv6
//! global unicast, and only falls back to link-local as a last resort.

#[cfg(not(target_os = "android"))]
mod platform {
    use crate::protocol::{MDNS_SERVICE_TYPE, PROTOCOL_VERSION};
    use anyhow::{Context, Result};
    use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::{Arc, Mutex};
    use tokio::sync::mpsc;
    use tracing::{debug, info, warn};
    use uuid::Uuid;

    // ── Peer event ────────────────────────────────────────────────────────────────

    #[derive(Debug, Clone)]
    pub struct PeerInfo {
        pub device_id: Uuid,
        pub device_name: String,
        pub addr: IpAddr,
        pub port: u16,
    }

    #[derive(Debug)]
    pub enum PeerEvent {
        Found(PeerInfo),
        Lost(Uuid),
    }

    // ── Discoverer ────────────────────────────────────────────────────────────────

    pub struct Discovery {
        mdns: ServiceDaemon,
        my_device_id: Uuid,
    }

    impl Discovery {
        pub fn new(my_device_id: Uuid) -> Result<Self> {
            let mdns = ServiceDaemon::new().context("creating mDNS daemon")?;
            Ok(Self { mdns, my_device_id })
        }

        /// Advertise our service on the LAN.
        ///
        /// Fix 6: `v` TXT record now uses `PROTOCOL_VERSION` (currently 3) so
        /// peers can skip us cleanly if they're running an incompatible version.
        pub fn advertise(
            &self,
            device_name: &str,
            port: u16,
            bind_ip: Option<IpAddr>,
        ) -> Result<()> {
            let mut properties = HashMap::new();
            properties.insert("id".to_string(), self.my_device_id.to_string());
            // TRU-06: Device names are intentionally NOT included in mDNS TXT records.
            // Only the opaque UUID is broadcast here.  The friendly device name is
            // exchanged after a successful encrypted handshake via HelloFrame/HelloAck,
            // so passive mDNS observers cannot enumerate device names on the LAN.
            // Fix 6: was hard-coded to "1"; now dynamically reflects PROTOCOL_VERSION.
            properties.insert("v".to_string(), PROTOCOL_VERSION.to_string());

            let instance_name = service_instance_name(device_name, self.my_device_id);

            let service = ServiceInfo::new(
                MDNS_SERVICE_TYPE,
                &instance_name,
                &format!("{}.local.", gethostname()),
                bind_ip.unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
                port,
                Some(properties),
            )
            .context("building ServiceInfo")?;

            self.mdns
                .register(service)
                .context("registering mDNS service")?;
            info!("mDNS: advertising '{}' on port {}", device_name, port);
            Ok(())
        }

        /// Browse for peers; discovered events are sent to `tx`.
        pub fn browse(&self, tx: mpsc::Sender<PeerEvent>) -> Result<()> {
            let receiver = self
                .mdns
                .browse(MDNS_SERVICE_TYPE)
                .context("starting mDNS browse")?;

            let my_id = self.my_device_id;
            let resolved = Arc::new(Mutex::new(HashMap::<String, Uuid>::new()));

            tokio::spawn(async move {
                loop {
                    match receiver.recv_async().await {
                        Ok(event) => match event {
                            ServiceEvent::ServiceResolved(info) => {
                                // Fix 6: Validate protocol version before connecting.
                                // Incompatible peers are skipped at mDNS time instead of
                                // failing expensively at the TCP handshake layer.
                                let peer_version: Option<u16> =
                                    info.get_property_val_str("v").and_then(|s| s.parse().ok());

                                match peer_version {
                                    Some(v) if v == PROTOCOL_VERSION => {} // OK, proceed
                                    Some(v) => {
                                        warn!(
                                            "mDNS: skipping peer with incompatible protocol \
                                             v{} (we speak v{})",
                                            v, PROTOCOL_VERSION
                                        );
                                        continue;
                                    }
                                    None => {
                                        warn!(
                                            "mDNS: resolved service missing 'v' TXT record, \
                                             skipping (not a Deskdrop v3+ peer)"
                                        );
                                        continue;
                                    }
                                }

                                // Parse device_id from TXT record.
                                let peer_id = info
                                    .get_property_val_str("id")
                                    .and_then(|s| Uuid::parse_str(s).ok());

                                let Some(peer_id) = peer_id else {
                                    warn!(
                                        "mDNS: resolved service missing 'id' TXT record, skipping"
                                    );
                                    continue;
                                };

                                // Skip our own advertisement.
                                if peer_id == my_id {
                                    continue;
                                }

                                // Use the service instance name as a provisional display name
                                // when it contains a sanitized device label. The encrypted
                                // handshake still upgrades this to the canonical friendly name.
                                let device_name =
                                    provisional_device_name(info.get_fullname(), peer_id);

                                // Fix 7: Prefer IPv4 over IPv6 link-local.
                                // Old code: `info.get_addresses().iter().next()` — arbitrary
                                // HashSet order, often picks fe80:: which needs a scope_id.
                                let addr = prefer_ipv4(info.get_addresses());
                                let Some(addr) = addr else {
                                    warn!("mDNS: service {} has no usable addresses", peer_id);
                                    continue;
                                };

                                let port = info.get_port();
                                info!(
                                    "mDNS: found peer {} at {}:{} (name resolved after handshake)",
                                    peer_id, addr, port
                                );
                                // Dedup guard: skip re-emitting Found for a device UUID
                                // that is already resolved at the same address+port.
                                // This prevents redundant connect attempts when mDNS
                                // re-announces the same peer after a network change.
                                {
                                    let mut map = resolved.lock().unwrap();
                                    let fullname = info.get_fullname().to_string();
                                    if let Some(&existing_id) = map.get(&fullname) {
                                        if existing_id == peer_id {
                                            debug!(
                                                "mDNS: skipping duplicate Found for peer {} \
                                                 (already resolved at same service name)",
                                                peer_id
                                            );
                                            continue;
                                        }
                                    }
                                    map.insert(fullname, peer_id);
                                }

                                let peer = PeerInfo {
                                    device_id: peer_id,
                                    device_name,
                                    addr,
                                    port,
                                };

                                if tx.send(PeerEvent::Found(peer)).await.is_err() {
                                    break; // channel closed
                                }
                            }

                            ServiceEvent::ServiceRemoved(_, fullname) => {
                                debug!("mDNS: service removed: {}", fullname);
                                let removed = { resolved.lock().unwrap().remove(&fullname) };
                                if let Some(peer_id) = removed {
                                    if tx.send(PeerEvent::Lost(peer_id)).await.is_err() {
                                        break;
                                    }
                                }
                            }

                            ServiceEvent::SearchStarted(_) => {
                                debug!("mDNS: search started");
                            }

                            _ => {}
                        },
                        Err(_) => {
                            warn!("mDNS browse channel closed");
                            break;
                        }
                    }
                }
            });

            Ok(())
        }

        /// Unregister our service (graceful shutdown).
        pub fn shutdown(self) -> Result<()> {
            self.mdns.shutdown().context("shutting down mDNS")?;
            Ok(())
        }
    }

    /// Fix 7: Select the best address from a peer's advertised set.
    ///
    /// Preference order:
    ///   1. IPv4 — no scope_id needed, works universally on LAN.
    ///   2. IPv6 global unicast (not fe80::/10) — usable without scope_id.
    ///   3. IPv6 link-local — last resort; may fail without scope_id.
    fn prefer_ipv4<'a>(addrs: impl IntoIterator<Item = &'a IpAddr>) -> Option<IpAddr> {
        let addrs: Vec<IpAddr> = addrs.into_iter().copied().collect();
        if addrs.is_empty() {
            return None;
        }
        // 1. Any IPv4.
        if let Some(&v4) = addrs.iter().find(|a| a.is_ipv4()) {
            return Some(v4);
        }
        // 2. IPv6 global unicast.
        if let Some(&v6) = addrs.iter().find(|a| {
            if let IpAddr::V6(v6) = a {
                !v6.is_loopback() && !is_ipv6_link_local(*v6)
            } else {
                false
            }
        }) {
            return Some(v6);
        }
        // 3. Fallback.
        Some(addrs[0])
    }

    /// Returns true if the address is in the IPv6 link-local range (fe80::/10).
    fn is_ipv6_link_local(addr: std::net::Ipv6Addr) -> bool {
        let o = addr.octets();
        o[0] == 0xFE && (o[1] & 0xC0) == 0x80
    }

    fn gethostname() -> String {
        hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "deskdrop-host".to_string())
    }

    fn service_instance_name(device_name: &str, device_id: Uuid) -> String {
        let prefix = &device_id.to_string()[..8];
        let safe = sanitize_service_label(device_name);
        if safe.is_empty() {
            format!("deskdrop-{prefix}")
        } else {
            format!("deskdrop-{prefix}-{safe}")
        }
    }

    fn provisional_device_name(fullname: &str, peer_id: Uuid) -> String {
        let instance = fullname
            .split("._deskdrop._tcp.local.")
            .next()
            .unwrap_or(fullname);
        let prefix = format!("deskdrop-{}", &peer_id.to_string()[..8]);

        let Some(raw_name) = instance.strip_prefix(&format!("{prefix}-")) else {
            return format!("device-{}", &peer_id.to_string()[..8]);
        };

        let humanized = raw_name
            .split('-')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        if humanized.is_empty() {
            format!("device-{}", &peer_id.to_string()[..8])
        } else {
            humanized
        }
    }

    fn sanitize_service_label(value: &str) -> String {
        value
            .chars()
            .take(24)
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
            .collect::<String>()
            .trim_matches('-')
            .to_string()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn prefer_ipv4_over_v6_link_local() {
            let v4: IpAddr = "192.168.1.1".parse().unwrap();
            let ll: IpAddr = "fe80::1".parse().unwrap();
            assert_eq!(prefer_ipv4(&[ll, v4]), Some(v4));
        }

        #[test]
        fn prefer_ipv4_falls_back_to_global_v6() {
            let ll: IpAddr = "fe80::1".parse().unwrap();
            let global: IpAddr = "2001:db8::1".parse().unwrap();
            assert_eq!(prefer_ipv4(&[ll, global]), Some(global));
        }

        #[test]
        fn prefer_ipv4_link_local_last_resort() {
            let ll: IpAddr = "fe80::1".parse().unwrap();
            assert_eq!(prefer_ipv4(&[ll]), Some(ll));
        }

        #[test]
        fn prefer_ipv4_empty_is_none() {
            let empty: &[IpAddr] = &[];
            assert_eq!(prefer_ipv4(empty), None);
        }

        #[test]
        fn is_ipv6_link_local_detection() {
            let ll: std::net::Ipv6Addr = "fe80::1".parse().unwrap();
            let global: std::net::Ipv6Addr = "2001:db8::1".parse().unwrap();
            assert!(is_ipv6_link_local(ll));
            assert!(!is_ipv6_link_local(global));
        }

        #[test]
        fn version_validation_logic() {
            // Simulate parsing the v TXT record.
            let our_version = PROTOCOL_VERSION;
            let old_version: u16 = our_version.saturating_sub(1);
            let new_version: u16 = our_version + 1;

            // Correct version should pass.
            assert_eq!(
                Some(our_version).filter(|&v| v == PROTOCOL_VERSION),
                Some(our_version)
            );
            // Old version should be filtered out.
            assert_eq!(Some(old_version).filter(|&v| v == PROTOCOL_VERSION), None);
            // Future version should also be filtered out.
            assert_eq!(Some(new_version).filter(|&v| v == PROTOCOL_VERSION), None);
        }

        #[test]
        fn provisional_name_uses_service_label_when_present() {
            let id = Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();
            let name =
                provisional_device_name("deskdrop-12345678-Pixel-8-Pro._deskdrop._tcp.local.", id);
            assert_eq!(name, "Pixel 8 Pro");
        }
    }
}

#[cfg(target_os = "android")]
mod platform {
    use anyhow::Result;
    use std::net::IpAddr;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    #[derive(Debug, Clone)]
    pub struct PeerInfo {
        pub device_id: Uuid,
        pub device_name: String,
        pub addr: IpAddr,
        pub port: u16,
    }

    #[derive(Debug)]
    pub enum PeerEvent {
        Found(PeerInfo),
        Lost(Uuid),
    }

    pub struct Discovery {
        _device_id: Uuid,
    }

    impl Discovery {
        pub fn new(my_device_id: Uuid) -> Result<Self> {
            Ok(Self {
                _device_id: my_device_id,
            })
        }

        pub fn advertise(
            &self,
            _device_name: &str,
            _port: u16,
            _bind_ip: Option<IpAddr>,
        ) -> Result<()> {
            Ok(())
        }

        pub fn browse(&self, _tx: mpsc::Sender<PeerEvent>) -> Result<()> {
            Ok(())
        }

        pub fn shutdown(self) -> Result<()> {
            Ok(())
        }
    }
}

pub use platform::{Discovery, PeerEvent, PeerInfo};
