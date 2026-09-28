//! Inbound pairing requests and responses, and QR auth.

use super::*;

pub(super) async fn handle(ctx: &InboundCtx, msg: AppMessage) -> Flow {
    let shared = &ctx.shared;
    let peer_id = ctx.peer_id;
    let peer_name = &ctx.peer_name;
    match msg {
        AppMessage::PairingRequest {
            origin_device,
            origin_device_name,
            pin: _req_pin,
        } => {
            ctx.touch_last_seen();

            // Self-healing trust: if we already trust this peer cryptographically,
            // and they are asking to pair again (perhaps they lost their app data),
            // automatically accept the request so they can trust us back.
            let is_trusted = shared
                .peer_manager
                .get(peer_id)
                .map(|p| p.trusted)
                .unwrap_or(false);
            if is_trusted {
                tracing::info!(peer_id = %peer_id, "Auto-accepting pairing request from already trusted device");
                let _ = ctx
                    .outbox_tx
                    .send(AppMessage::PairingResponse {
                        origin_device: shared.config.device_id,
                        accepted: true,
                    })
                    .await;
                return Flow::Continue;
            }

            let _ = shared.peer_manager.set_pairing_requested(peer_id, true);

            // Re-emit PairingRequested with the REAL name and PIN so the UI updates
            let pin = ctx
                .session_pin
                .clone()
                .or_else(|| shared.peer_manager.get(peer_id).and_then(|p| p.pairing_pin))
                .unwrap_or_else(|| "------".to_string());
            let _ = shared
                .peer_manager
                .set_pairing_pin(peer_id, Some(pin.clone()));
            let _ = shared
                .event_tx
                .send(EngineEvent::PairingRequested {
                    device_id: origin_device,
                    device_name: origin_device_name.clone(),
                    pin,
                })
                .await;

            let _ = shared
                .event_tx
                .send(EngineEvent::PairingRequest {
                    device_id: origin_device,
                    device_name: origin_device_name,
                })
                .await;
        }
        AppMessage::PairingResponse {
            origin_device,
            accepted,
        } => {
            ctx.touch_last_seen();

            // CRIT-03 FIX: Only process PairingResponse if:
            //   1. The origin_device matches the actual session peer_id
            //      (prevents a connected peer from spoofing trust for a different device).
            //   2. We previously sent a PairingRequest to this peer
            //      (tracked via the pairing_requested flag set in respond_to_pairing).
            //   3. The peer is not already trusted (prevents re-trust of revoked peers).
            if origin_device != peer_id {
                tracing::warn!(
                    peer_id = %peer_id,
                    claimed_device = %origin_device,
                    "ignoring PairingResponse: origin_device does not match session peer"
                );
                return Flow::Continue;
            }

            // Check that we actually initiated pairing with this peer.
            // The `pairing_requested` flag is set to true in observe_trust()
            // when we emit PairingRequested, and cleared in respond_to_pairing().
            // A remote peer sending an unsolicited PairingResponse is rejected.
            let we_requested_pairing = shared
                .peer_manager
                .get(peer_id)
                .map(|p| p.outgoing_pairing_waiting)
                .unwrap_or(false);
            let we_already_trust_them = shared
                .peer_manager
                .get(peer_id)
                .map(|p| p.trusted)
                .unwrap_or(false);

            if !we_requested_pairing && !we_already_trust_them {
                tracing::warn!(
                    peer_id = %peer_id,
                    "ignoring unsolicited PairingResponse — no pending pairing request and not already trusted"
                );
                return Flow::Continue;
            }

            // Clear the pairing_requested flag now that we've received the response.
            let _ = shared.peer_manager.set_pairing_requested(peer_id, false);
            let _ = shared
                .peer_manager
                .set_outgoing_pairing_waiting(peer_id, false);

            if !accepted {
                tracing::info!(peer_id = %peer_id, "peer rejected pairing request");
                let _ = shared.peer_manager.set_pairing_pin(peer_id, None);
                let _ = shared
                    .event_tx
                    .send(EngineEvent::PairingResponse {
                        device_id: origin_device,
                        accepted,
                    })
                    .await;
                return Flow::Disconnect("peer rejected pairing request".to_string());
            } else {
                // ── CRITICAL: Establish mutual trust ──────────────
                // The remote peer accepted our pairing request and
                // already trusts us (set in respond_to_pairing).
                // We must trust them back so the connection is fully
                // bidirectional — otherwise the dashboard shows
                // "not connected" and file transfers fail.
                tracing::info!(peer_id = %peer_id, "peer accepted pairing — establishing mutual trust");
                if !we_already_trust_them {
                    let mut trust = shared.trust.lock().await;
                    let _ = trust.trust_peer(peer_id);
                    let _ = shared.peer_manager.update_trust(peer_id, true);
                }
                let _ = shared.peer_manager.set_auto_connect(peer_id, true);
                let _ = shared.peer_manager.set_pairing_pin(peer_id, None);

                // Emit PeerConnected so the UI updates immediately.
                let _ = shared
                    .event_tx
                    .send(EngineEvent::PeerConnected {
                        device_id: peer_id,
                        device_name: peer_name.clone(),
                        addr: ctx.endpoint,
                        trusted: true,
                    })
                    .await;
            }
            let _ = shared
                .event_tx
                .send(EngineEvent::PairingResponse {
                    device_id: origin_device,
                    accepted,
                })
                .await;
        }
        AppMessage::QrAuth { token } => {
            let valid = {
                let mut stored = shared.qr_auth_token.lock().await;
                if let Some(t) = stored.take() {
                    t.token == token && t.expires_at > std::time::Instant::now()
                } else {
                    false
                }
            };

            if valid {
                tracing::info!(peer_id = %peer_id, "peer provided valid QR auth token — establishing mutual trust");
                let _ = shared.peer_manager.set_pairing_requested(peer_id, false);
                let _ = shared
                    .peer_manager
                    .set_outgoing_pairing_waiting(peer_id, false);

                let mut trust = shared.trust.lock().await;
                let _ = trust.trust_peer(peer_id);
                let _ = shared.peer_manager.update_trust(peer_id, true);
                let _ = shared.peer_manager.set_auto_connect(peer_id, true);
                let _ = shared.peer_manager.set_pairing_pin(peer_id, None);

                // Emit PeerConnected so the UI updates immediately.
                let _ = shared
                    .event_tx
                    .send(EngineEvent::PeerConnected {
                        device_id: peer_id,
                        device_name: peer_name.clone(),
                        addr: ctx.endpoint,
                        trusted: true,
                    })
                    .await;
            } else {
                tracing::warn!(peer_id = %peer_id, "peer provided invalid QR auth token");
            }
        }
        _ => unreachable!("dispatch_inbound routed a non-pairing message here"),
    }
    Flow::Continue
}
