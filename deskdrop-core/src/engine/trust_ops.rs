//! Trust and device management: pairing, QR auth, approve/reject/revoke,
//! rename, forget, and per-peer sync and auto-connect.

use super::*;

impl Engine {
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

    pub async fn generate_qr_token(&self) -> String {
        use rand::RngCore;
        let mut bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut bytes);
        let token = hex::encode(bytes);
        *self.shared.qr_auth_token.lock().await = Some(QrAuthToken {
            token: token.clone(),
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(60),
        });
        token
    }

    pub async fn send_qr_auth(&self, target_device: Uuid, token: String) {
        let msg = AppMessage::QrAuth { token };
        let peers = self.shared.peer_manager.all_connected_senders();
        if let Some(tx) = peers
            .into_iter()
            .find(|(id, _)| *id == target_device)
            .map(|(_, tx)| tx)
        {
            let _ = tx.send(msg).await;
        } else {
            if let Some(peer) = self.shared.peer_manager.get(target_device) {
                let addrs = peer.socket_addrs();
                if !addrs.is_empty() {
                    let shared = self.shared.clone();
                    tokio::spawn(async move {
                        if let Ok(()) = connect_loop(
                            shared.clone(),
                            addrs,
                            Some(target_device),
                            DiscoverySource::Manual,
                        )
                        .await
                        {
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                            let peers = shared.peer_manager.all_connected_senders();
                            if let Some(tx) = peers
                                .into_iter()
                                .find(|(id, _)| *id == target_device)
                                .map(|(_, tx)| tx)
                            {
                                let _ = tx.send(msg).await;
                            }
                        }
                    });
                }
            }
        }
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
            let _ = self.shared.peer_manager.set_auto_connect(device_id, true);

            // Push local battery and network status to the newly trusted peer.
            let mut target_tx: Option<tokio::sync::mpsc::Sender<crate::protocol::AppMessage>> =
                None;
            for (id, tx) in self.shared.peer_manager.active_senders() {
                if id == device_id {
                    target_tx = Some(tx);
                    break;
                }
            }
            if let Some(tx) = target_tx {
                let sh = self.shared.clone();
                tokio::spawn(async move {
                    let battery_val = *sh.device_status.local_battery.lock().unwrap();
                    if let Some((level, charging)) = battery_val {
                        let _ = tx
                            .send(AppMessage::BatteryStatus {
                                level,
                                charging,
                                origin_device: sh.config.device_id,
                                origin_device_name: sh.config.device_name.clone(),
                            })
                            .await;
                    }
                    let net_val = sh.device_status.local_network.lock().unwrap().clone();
                    if let Some(net) = net_val {
                        let _ = tx
                            .send(AppMessage::NetworkStatus {
                                network_type: net,
                                origin_device: sh.config.device_id,
                                origin_device_name: sh.config.device_name.clone(),
                            })
                            .await;
                    }
                });
            }
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
            let _ = self.disconnect_peer(device_id).await;
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

    pub async fn unreject_peer(&self, device_id: Uuid) -> Result<bool> {
        let changed = {
            let mut trust = self.shared.trust.lock().await;
            trust.unreject_peer(device_id)?
        };
        Ok(changed)
    }

    pub async fn send_pairing_request(&self, target_device: Uuid) {
        // Clear any previous Rejected or Revoked state so the outbound connection isn't blocked.
        let _ = self.unreject_peer(target_device).await;

        // Mark that WE initiated a pairing request so the PairingResponse
        // handler accepts the response (CRIT-03 anti-spoof check).
        let _ = self
            .shared
            .peer_manager
            .set_outgoing_pairing_waiting(target_device, true);

        let live_tx = self
            .shared
            .peer_manager
            .all_connected_senders()
            .into_iter()
            .find(|(id, _)| *id == target_device)
            .map(|(_, tx)| tx);

        match live_tx {
            Some(tx) => {
                let _ = tx
                    .send(pairing_request_message(&self.shared, target_device))
                    .await;
            }
            None => {
                // Dial in the background instead of awaiting connect_loop's
                // full retry/backoff here: that blocked the IPC caller for
                // seconds (the Windows client gave up and reported failure)
                // and silently dropped the request whenever the session came
                // up some other way (e.g. the peer dialed us). The request is
                // delivered by register_session as soon as any session with
                // this peer goes live, since outgoing_pairing_waiting is set.
                if let Some(peer) = self.shared.peer_manager.get(target_device) {
                    let addrs = peer.socket_addrs();
                    if !addrs.is_empty() {
                        let shared = self.shared.clone();
                        tokio::spawn(async move {
                            if let Err(err) = connect_loop(
                                shared,
                                addrs,
                                Some(target_device),
                                DiscoverySource::Manual,
                            )
                            .await
                            {
                                warn!(peer_id = %target_device, error = %err, "pairing connect failed");
                            }
                        });
                    }
                }
            }
        }

        let peer = self.shared.peer_manager.get(target_device);
        let pin = peer
            .as_ref()
            .and_then(|p| p.pairing_pin.clone())
            .unwrap_or_else(|| "------".to_string());
        let device_name = peer
            .map(|p| p.friendly_name)
            .unwrap_or_else(|| "Unknown device".to_string());
        let _ = self
            .shared
            .event_tx
            .send(EngineEvent::OutgoingPairingWaiting {
                device_id: target_device,
                device_name,
                pin,
            })
            .await;
    }

    pub async fn initiate_pairing(&self, target_device: Uuid) -> Result<()> {
        self.send_pairing_request(target_device).await;
        Ok(())
    }

    pub async fn report_discovered_peer(
        &self,
        device_id: Uuid,
        device_name: String,
        ip: String,
        port: u16,
    ) -> Result<()> {
        let ip_addr = ip.parse::<std::net::IpAddr>().context("invalid IP")?;
        let endpoint = std::net::SocketAddr::new(ip_addr, port);
        let _ = self.shared.peer_manager.upsert_peer(
            device_id,
            device_name,
            endpoint,
            false,
            crate::peer_manager::DiscoverySource::Manual,
        );
        Ok(())
    }
    pub async fn respond_to_pairing(&self, requester_device: Uuid, accepted: bool) -> Result<()> {
        let _ = self
            .shared
            .peer_manager
            .set_pairing_requested(requester_device, false);
        if accepted {
            // Trust them persistently
            self.trust_peer(requester_device).await?;
        }
        let msg = AppMessage::PairingResponse {
            origin_device: self.shared.config.device_id,
            accepted,
        };
        let peers = self.shared.peer_manager.all_connected_senders();
        if let Some(tx) = peers
            .into_iter()
            .find(|(id, _)| *id == requester_device)
            .map(|(_, tx)| tx)
        {
            let _ = tx.send(msg).await;
        }
        if !accepted {
            // Reject the peer in the trust store so they don't auto-reconnect
            // and re-prompt endlessly. observe_trust checks for Rejected state
            // and bails, preventing the re-prompt loop.
            // reject_peer also disconnects the session internally.
            let _ = self.reject_peer(requester_device).await;
        }
        Ok(())
    }

    /// Pause Sync: keep connection alive, suppress clipboard data flow.
    pub async fn pause_sync_peer(&self, device_id: Uuid) -> Result<bool> {
        self.shared.peer_manager.set_sync_enabled(device_id, false)
    }

    /// Resume Sync: re-enable clipboard data flow.
    pub async fn resume_sync_peer(&self, device_id: Uuid) -> Result<bool> {
        self.shared.peer_manager.set_sync_enabled(device_id, true)
    }

    /// Forget Device: remove persistent pairing and revoke trust.
    pub async fn forget_device(&self, device_id: Uuid) -> Result<bool> {
        let found = self.shared.peer_manager.forget_device(device_id)?;
        if found {
            let _ = self.shared.trust.lock().await.revoke_peer(device_id);
            // Drain pending RPC waiters for forgotten device
            drain_remote_waiters(&self.shared, device_id).await;
            // Disconnect the session — device will not auto-reconnect
            let session = self.shared.peer_manager.shutdown_peer_session(device_id)?;
            if let Some(session) = session {
                if let Some(shutdown_tx) = session.shutdown_tx {
                    let _ = shutdown_tx.send(crate::peer_manager::SessionShutdown {
                        reason: "device forgotten".to_string(),
                        send_bye: true,
                        explicit_disconnect: false,
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
}
