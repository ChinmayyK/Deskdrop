//! mDNS-SD (DNS Service Discovery) — advertise and browse for ClipRelay peers.
//!
//! Each running ClipRelay daemon:
//!   1. Registers a `_cliprelay._tcp.local.` service with its TCP port.
//!   2. Continuously browses for other `_cliprelay._tcp.local.` services.
//!   3. When a new peer appears, the `PeerEvent::Found` event fires.
//!   4. When a peer disappears, `PeerEvent::Lost` fires.
//!
//! The caller then initiates a TCP handshake to Found peers.

use crate::protocol::MDNS_SERVICE_TYPE;
use anyhow::{Context, Result};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::net::IpAddr;
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
    pub fn advertise(&self, device_name: &str, port: u16, bind_ip: Option<IpAddr>) -> Result<()> {
        // TXT record carries our device_id so peers know who we are before TCP.
        let mut properties = HashMap::new();
        properties.insert("id".to_string(), self.my_device_id.to_string());
        properties.insert("name".to_string(), device_name.to_string());
        properties.insert("v".to_string(), "1".to_string()); // protocol version

        // Instance name must be unique — use device_id prefix.
        let instance_name = format!("cliprelay-{}", &self.my_device_id.to_string()[..8]);

        let service = ServiceInfo::new(
            MDNS_SERVICE_TYPE,
            &instance_name,
            &format!("{}.local.", gethostname()),
            bind_ip.unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)),
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
                    Ok(event) => {
                        match event {
                            ServiceEvent::ServiceResolved(info) => {
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

                                let device_name = info
                                    .get_property_val_str("name")
                                    .unwrap_or("Unknown Device")
                                    .to_string();

                                // Pick the first resolved address.
                                let Some(&addr) = info.get_addresses().iter().next() else {
                                    warn!("mDNS: service {} has no addresses", peer_id);
                                    continue;
                                };

                                let port = info.get_port();
                                info!("mDNS: found peer '{}' at {}:{}", device_name, addr, port);
                                resolved
                                    .lock()
                                    .unwrap()
                                    .insert(info.get_fullname().to_string(), peer_id);

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
                        }
                    }
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

fn gethostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "cliprelay-host".to_string())
}
