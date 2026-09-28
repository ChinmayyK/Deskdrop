//! A live peer session: registration, the reader/writer loop, and
//! dispatch of every inbound AppMessage.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn register_session(
    shared: EngineShared,
    stream: TcpStream,
    endpoint: SocketAddr,
    peer_id: Uuid,
    peer_name: String,
    session: crate::crypto::SessionKey,
    trusted: bool,
    peer_trusts_us: Option<bool>,
    discovery: DiscoverySource,
    session_pin: Option<String>,
    is_outbound: bool,
) -> Result<()> {
    // File chunks get their own small queue: 8 × 4 MB = 32 MB waiting to be
    // encrypted, on top of the kernel socket buffer. That is enough to keep
    // any LAN link busy, and the backpressure stops the reader from racing
    // ahead of the network and pinning hundreds of MB on a phone.
    let (outbox_tx, mut outbox_rx) = mpsc::channel::<AppMessage>(64);
    let (file_outbox_tx, mut file_outbox_rx) = mpsc::channel::<AppMessage>(8);
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<SessionShutdown>();
    match shared
        .peer_manager
        .upsert_peer(peer_id, peer_name.clone(), endpoint, trusted, discovery)
    {
        Ok(_) => {}
        Err(e) => {
            warn!("peer discovery connect failed error={:?}", e);
            return Err(e);
        }
    }
    let (session_id, replaced, rejected_new) = shared.peer_manager.replace_live_session(
        shared.config.device_id,
        peer_id,
        is_outbound,
        endpoint,
        outbox_tx.clone(),
        file_outbox_tx.clone(),
        shutdown_tx,
    )?;

    if rejected_new {
        tracing::debug!(
            "Session with {} rejected by dedup (we already have the winning session)",
            peer_id
        );
        return Ok(());
    }

    // The PIN is derived per handshake, and a simultaneous dial runs two
    // handshakes with different PINs. Recording it only for the session
    // that won the tie-break keeps both devices showing the same code;
    // writing it before dedup let each side keep a different loser's PIN.
    if let Some(pin) = session_pin.clone() {
        let _ = shared
            .peer_manager
            .set_pairing_pin(peer_id, Some(pin.clone()));
        if let Some(they_trust_us) = peer_trusts_us {
            reconcile_one_sided_trust(&shared, peer_id, &peer_name, trusted, they_trust_us, pin);
        }
    }

    let was_connected = replaced.is_some();
    if let Some(replaced) = replaced {
        if let Some(old_shutdown) = replaced.shutdown_tx {
            let _ = old_shutdown.send(SessionShutdown {
                reason: format!("session migrated to {}", endpoint),
                send_bye: false,
                explicit_disconnect: false,
            });
        }
    }

    // A session swap (duplicate-dial tie-break, or a fresh stream replacing
    // a stale one) is invisible to the user: the peer never went offline.
    if !was_connected {
        let _ = shared.event_tx.try_send(EngineEvent::PeerConnected {
            device_id: peer_id,
            device_name: peer_name.clone(),
            addr: endpoint,
            trusted,
        });

        let feed = shared.activity.clone();
        let name = peer_name.clone();
        tokio::spawn(async move {
            feed.lock().await.record_peer_connected(peer_id, name);
        });
    }

    // The user asked to pair before this session existed (or on a session
    // that has since been replaced), or reconcile_one_sided_trust found the
    // peer no longer trusts us: deliver the request now.
    if shared
        .peer_manager
        .get(peer_id)
        .is_some_and(|p| p.outgoing_pairing_waiting)
    {
        let _ = outbox_tx.try_send(pairing_request_message(&shared, peer_id));
    }

    // A peer's "clipboard sharing off" notice only lives as long as the
    // session it arrived on: it isn't resent on its own, so a stale flag from
    // an old session would block broadcasts to it forever. Reset here; a
    // peer that has sharing off re-announces it right below.
    let _ = shared.peer_manager.set_remote_sync_enabled(peer_id, true);

    // Push local battery and network status to the newly connected peer if trusted.
    if trusted {
        let outbox = outbox_tx.clone();
        let sh = shared.clone();
        tokio::spawn(async move {
            let sharing_on = sh.settings.lock().unwrap().sync_enabled;
            if !sharing_on {
                let _ = outbox
                    .send(AppMessage::DeviceSyncState { enabled: false })
                    .await;
            }
            let battery_val = *sh.device_status.local_battery.lock().unwrap();
            if let Some((level, charging)) = battery_val {
                let _ = outbox
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
                let _ = outbox
                    .send(AppMessage::NetworkStatus {
                        network_type: net,
                        origin_device: sh.config.device_id,
                        origin_device_name: sh.config.device_name.clone(),
                    })
                    .await;
            }
            let storage_val = *sh.device_status.local_storage.lock().unwrap();
            if let Some((images, videos, apps, free, total)) = storage_val {
                let _ = outbox
                    .send(AppMessage::StorageStatus {
                        images_bytes: images,
                        videos_bytes: videos,
                        apps_bytes: apps,
                        free_bytes: free,
                        total_bytes: total,
                        origin_device: sh.config.device_id,
                        origin_device_name: sh.config.device_name.clone(),
                    })
                    .await;
            }
        });
    }

    // If this peer is reconnecting, re-announce any unfinished outbound
    // transfers so the receiver can respond with resume_from_chunk.
    {
        let pending_shared = shared.clone();
        let pending_outbox = outbox_tx.clone();
        tokio::spawn(async move {
            let pending = pending_shared
                .file_transfers
                .lock()
                .await
                .pending_outbound_announcements_for(peer_id);
            for meta in pending {
                if pending_outbox
                    .send(AppMessage::FileTransferAnnounce { meta })
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    // MED-02: wrap the session task in a JoinHandle watcher so that panics
    // are logged rather than silently swallowed by the Tokio runtime.
    let panic_peer_name = peer_name.clone();
    let session_outbox_tx = outbox_tx.clone();
    let session_handle = tokio::spawn(async move {
        let (mut sess_tx, mut sess_rx) = PeerSession {
            stream,
            session,
            peer_device_id: peer_id,
            peer_device_name: peer_name.clone(),
        }
        .split();
        let mut heartbeat = tokio::time::interval(shared.config.heartbeat_interval);
        // After a suspend, the default Burst behaviour fires every missed
        // tick back to back - a pointless ping storm on wake.
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let last_seen = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        ));
        let ping_sent_at = std::sync::Arc::new(std::sync::Mutex::new(None::<std::time::Instant>));
        let peer_sleeping = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let rx_last_seen = last_seen.clone();
        let rx_ping_sent_at = ping_sent_at.clone();
        let rx_peer_sleeping = peer_sleeping.clone();
        let rx_shared = shared.clone();
        let rx_peer_name = peer_name.clone();
        let rx_session_outbox_tx = session_outbox_tx.clone();
        let rx_peer_id = peer_id;
        let rx_session_pin = session_pin.clone();

        enum DiskTaskMsg {
            Chunk {
                transfer_id: [u8; 16],
                chunk_index: u32,
                offset: u64,
                padding: usize,
                data: Vec<u8>,
            },
            Complete {
                transfer_id: [u8; 16],
                sha256_checksum: String,
            },
        }

        let (disk_tx, mut disk_rx) = tokio::sync::mpsc::channel::<DiskTaskMsg>(8);
        let mut last_disk_prog_emit: std::collections::HashMap<[u8; 16], std::time::Instant> =
            std::collections::HashMap::new();

        let dw_shared = shared.clone();
        let dw_event_tx = shared.event_tx.clone();
        let dw_outbox_tx = session_outbox_tx.clone();
        let dw_peer_id = peer_id;
        let dw_peer_name = peer_name.clone();
        tokio::spawn(async move {
            while let Some(msg) = disk_rx.recv().await {
                match msg {
                    DiskTaskMsg::Chunk {
                        transfer_id,
                        chunk_index,
                        offset,
                        padding,
                        data,
                    } => {
                        let io_ctx = {
                            let mut mgr = dw_shared.file_transfers.lock().await;
                            if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                                t.take_io_context()
                            } else {
                                None
                            }
                        };

                        if let Some((mut file, mut hasher, last_offset)) = io_ctx {
                            let data_len = data.len();
                            let res = tokio::task::spawn_blocking(move || {
                                use sha2::Digest;
                                use std::io::{Seek, SeekFrom, Write};

                                if last_offset != offset {
                                    if let Err(e) = file.seek(SeekFrom::Start(offset)) {
                                        return Err(anyhow::anyhow!("seek error: {}", e));
                                    }
                                }
                                if let Err(e) = file.write_all(&data) {
                                    return Err(anyhow::anyhow!("write error: {}", e));
                                }
                                hasher.update(&data);
                                if padding > 0 {
                                    hasher.update(vec![0u8; padding]);
                                }
                                let new_offset = offset + data.len() as u64;
                                Ok::<_, anyhow::Error>((file, hasher, new_offset))
                            })
                            .await
                            .unwrap();

                            match res {
                                Ok((file, hasher, new_offset)) => {
                                    let mut mgr = dw_shared.file_transfers.lock().await;
                                    if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                                        t.restore_io_context(file, hasher, new_offset);
                                        let prog = t.commit_chunk(chunk_index, data_len);
                                        let should_ack = t.should_ack();
                                        let file_name = t.meta.file_name.clone();
                                        drop(mgr);

                                        let now = std::time::Instant::now();
                                        let last = last_disk_prog_emit
                                            .get(&transfer_id)
                                            .copied()
                                            .unwrap_or_else(|| {
                                                now.checked_sub(std::time::Duration::from_secs(1))
                                                    .unwrap()
                                            });
                                        if now.duration_since(last).as_millis() >= 100
                                            || prog.percent == 100
                                        {
                                            last_disk_prog_emit.insert(transfer_id, now);
                                            let _ = dw_event_tx.try_send(
                                                EngineEvent::FileTransferProgress {
                                                    transfer_id,
                                                    from_device: dw_peer_id,
                                                    file_name,
                                                    percent: prog.percent,
                                                    bytes_received: prog.bytes_received,
                                                    total_bytes: prog.total_bytes,
                                                    speed_bps: prog.speed_bps,
                                                    eta_secs: prog.eta_secs,
                                                },
                                            );
                                        }

                                        if should_ack {
                                            let _ = dw_outbox_tx
                                                .send(AppMessage::FileChunkAck {
                                                    transfer_id,
                                                    last_confirmed_chunk: chunk_index,
                                                })
                                                .await;
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Disk I/O error: {}", e);
                                    let mut mgr = dw_shared.file_transfers.lock().await;
                                    mgr.cancel_inbound(&transfer_id, "disk i/o error");
                                }
                            }
                        } else {
                            tracing::error!(
                                "Missing io_ctx for chunk {} of transfer {:?}",
                                chunk_index,
                                transfer_id
                            );
                        }
                    }
                    DiskTaskMsg::Complete {
                        transfer_id,
                        sha256_checksum,
                    } => {
                        let file_handle = {
                            let mut mgr = dw_shared.file_transfers.lock().await;
                            mgr.get_inbound_mut(&transfer_id)
                                .and_then(|t| t.file_handle.take())
                        };

                        if let Some(mut file) = file_handle {
                            let _ = tokio::task::spawn_blocking(move || {
                                use std::io::Write;
                                let _ = file.flush();
                                let _ = file.get_ref().sync_all();
                            })
                            .await;
                        }

                        let result = {
                            let mut mgr = dw_shared.file_transfers.lock().await;
                            if let Some(transfer) = mgr.get_inbound_mut(&transfer_id) {
                                let file_name = transfer.meta.file_name.clone();
                                let file_bytes = transfer.meta.size_bytes;
                                match transfer.finalize(sha256_checksum.clone()) {
                                    Ok(dest) => Ok((dest, file_name, file_bytes)),
                                    Err(e) => Err(e.to_string()),
                                }
                            } else {
                                Err("transfer not found".into())
                            }
                        };
                        match result {
                            Ok((dest, file_name, file_bytes)) => {
                                dw_shared
                                    .file_transfers
                                    .lock()
                                    .await
                                    .remove_inbound(&transfer_id);
                                let hex_tid = hex::encode(transfer_id);
                                let dest_path_str = dest.to_string_lossy().to_string();
                                dw_shared
                                    .activity
                                    .lock()
                                    .await
                                    .record_file_transfer_complete(
                                        dw_peer_id,
                                        dw_peer_name.clone(),
                                        file_name.clone(),
                                        file_bytes,
                                        hex_tid,
                                        Some(dest_path_str),
                                    );
                                let _ = dw_outbox_tx
                                    .send(AppMessage::FileTransferCompleteAck {
                                        transfer_id,
                                        success: true,
                                        error: None,
                                    })
                                    .await;
                                let _ = dw_event_tx
                                    .send(EngineEvent::FileTransferComplete {
                                        transfer_id,
                                        from_device: dw_peer_id,
                                        from_name: dw_peer_name.clone(),
                                        file_name,
                                        dest_path: dest,
                                    })
                                    .await;
                            }
                            Err(e) => {
                                {
                                    let mut mgr = dw_shared.file_transfers.lock().await;
                                    if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                                        t.status = crate::file_transfer::TransferStatus::Failed;
                                    }
                                }
                                let hex_tid = hex::encode(transfer_id);
                                dw_shared.activity.lock().await.record_file_transfer_failed(
                                    dw_peer_id,
                                    dw_peer_name.clone(),
                                    None,
                                    hex_tid,
                                    e.clone(),
                                );
                                let _ = dw_outbox_tx
                                    .send(AppMessage::FileTransferCompleteAck {
                                        transfer_id,
                                        success: false,
                                        error: Some(e.clone()),
                                    })
                                    .await;
                                let _ = dw_event_tx
                                    .send(EngineEvent::FileTransferFailed {
                                        transfer_id,
                                        from_device: dw_peer_id,
                                        reason: e,
                                    })
                                    .await;
                            }
                        }
                        pump_transfer_queue(&dw_shared).await;
                    }
                }
            }
        });

        let rx_disk_tx = disk_tx.clone();
        let mut rx_task = tokio::spawn(async move {
            let touch_last_seen = || {
                rx_last_seen.store(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                    std::sync::atomic::Ordering::Relaxed,
                );
            };
            let shared = rx_shared;
            let peer_name = rx_peer_name;
            let peer_id = rx_peer_id;
            loop {
                let result = sess_rx.recv().await;
                match result {
                    Ok(AppMessage::ClipboardPush {
                        seq,
                        mut content,
                        origin_device,
                        origin_device_name,
                        relay_path,
                    }) => {
                        touch_last_seen();
                        if shared
                            .peer_manager
                            .get(peer_id)
                            .map(|peer| peer.is_sync_eligible())
                            .unwrap_or(false)
                        {
                            let _ = shared.peer_manager.update_last_sync(peer_id);
                            let display_name = if origin_device_name.is_empty() {
                                peer_name.clone()
                            } else {
                                origin_device_name.clone()
                            };

                            // Run smart clipboard transformers (URL UTM parameter stripping, whitespace cleaning)
                            crate::transformer::TransformerPipeline::default_pipeline()
                                .transform(std::sync::Arc::make_mut(&mut content));

                            // --- Clipboard Security & Throttling ---
                            let payload_size = match &*content {
                                ClipboardContent::Text(t) => t.len(),
                                ClipboardContent::Image { data, .. } => data.len(),
                                ClipboardContent::File { data, .. } => data.len(),
                            };

                            // Limit sizes to prevent OOM
                            const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024;
                            const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;

                            let allowed = match &*content {
                                ClipboardContent::Text(t) => t.len() <= MAX_TEXT_BYTES,
                                ClipboardContent::Image { data, .. } => {
                                    data.len() <= MAX_IMAGE_BYTES
                                }
                                ClipboardContent::File { data, .. } => {
                                    data.len() <= MAX_IMAGE_BYTES
                                }
                            };

                            if !allowed {
                                tracing::warn!(peer_id = %peer_id, size = payload_size, "dropped oversized clipboard payload");
                                continue;
                            }

                            // Run inbound payload through the FilterChain (e.g. executable blocking, etc.)
                            let filter_chain = crate::filter::FilterChain::from_settings(
                                &shared.settings.lock().unwrap(),
                            );
                            if let crate::filter::Verdict::Deny { reason } =
                                filter_chain.run(&content)
                            {
                                tracing::warn!(peer_id = %peer_id, reason, "inbound clipboard payload denied by filter");
                                continue;
                            }

                            // ── Timeline-first clipboard UX ───────────────
                            let hash = hash_content(&content);
                            let hash_hex = hex::encode(hash);

                            // ── Deduplicator Check ───────────────
                            let should_apply = {
                                let mut dedup = shared.dedup.lock().await;
                                dedup.should_apply(origin_device, hash)
                            };

                            if !should_apply {
                                tracing::debug!("suppressing inbound clipboard push (dedup)");
                                // It is either an echo of our own send, or a duplicate from a second peer.
                                // Acknowledge it, but skip all local UI/clipboard updates.
                                let _ = rx_session_outbox_tx
                                    .send(AppMessage::ClipboardAck { seq })
                                    .await;
                                continue;
                            }

                            // The global "Share clipboard" switch. It used to be
                            // saved and never read, so turning it off changed
                            // nothing. Off: remote clips still land in history
                            // (so they can be applied by hand) but never
                            // overwrite the local clipboard or get relayed on.
                            let sharing_on = shared.settings.lock().unwrap().sync_enabled;
                            let auto_apply = sharing_on
                                && shared
                                    .apply_policy
                                    .lock()
                                    .await
                                    .should_auto_apply(origin_device);

                            // Record in activity feed.
                            let activity_id = {
                                if let ClipboardContent::Text(ref text) = *content {
                                    shared
                                        .clipboard_store
                                        .lock()
                                        .await
                                        .insert(hash_hex.clone(), text.clone());
                                }
                                let mut feed = shared.activity.lock().await;
                                match &*content {
                                    ClipboardContent::Text(ref text) => feed
                                        .record_remote_clipboard_text(
                                            origin_device,
                                            display_name.clone(),
                                            text,
                                            hash_hex.clone(),
                                            relay_path.clone(),
                                        ),
                                    ClipboardContent::Image { mime, data } => feed
                                        .record_remote_clipboard_image(
                                            origin_device,
                                            display_name.clone(),
                                            mime,
                                            data.len() as u64,
                                            hash_hex.clone(),
                                            relay_path.clone(),
                                        ),
                                    ClipboardContent::File { name, data } => feed
                                        .record_file_transfer_started(
                                            origin_device,
                                            display_name.clone(),
                                            name.clone(),
                                            data.len() as u64,
                                            hash_hex.clone(),
                                            false,
                                        ),
                                }
                            };

                            // If auto-applying, mark immediately applied.
                            if auto_apply {
                                let mut feed = shared.activity.lock().await;
                                feed.record_clipboard_applied(
                                    origin_device,
                                    display_name.clone(),
                                    hash_hex.clone(),
                                );
                            }

                            // Wrap content in Arc here so all downstream users — the
                            // EngineEvent and every relay-fanout hop — share one heap
                            // allocation instead of N independent clones (MED-01).
                            // (content is already Arc<ClipboardContent>)
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::ClipboardReceived {
                                    from_device: origin_device,
                                    from_name: display_name.clone(),
                                    content: content.clone(),
                                    auto_applied: auto_apply,
                                    relay_path: relay_path.clone(),
                                    activity_id,
                                })
                                .await;
                            let _ = rx_session_outbox_tx
                                .send(AppMessage::ClipboardAck { seq })
                                .await;

                            // Persist the incoming item to history.
                            {
                                let max_bytes =
                                    shared.settings.lock().unwrap().max_history_text_bytes;
                                let source = display_name.clone();
                                let _ = shared
                                    .history
                                    .lock()
                                    .await
                                    .push_with_options(&content, source, max_bytes);
                            }

                            // ── Mesh fanout relay ──────────────────────────
                            // If we received from a direct peer but there are other
                            // peers in the mesh, relay onwards (excluding origin + seen).
                            // Wrap content in Arc so each relay hop shares the same
                            // heap allocation instead of cloning the full payload
                            // (MED-01 — AppMessage::clone on relay hops).
                            let fanout_peers = if sharing_on {
                                shared.peer_manager.active_senders()
                            } else {
                                Vec::new()
                            };
                            let mut router = shared.mesh_router.lock().await;
                            // shared_content is already Arc-wrapped above; no further
                            // full clone needed here — each fan-out is a pointer clone
                            // plus one cheap metadata-struct clone (MED-01).
                            for (fp_id, fp_tx) in fanout_peers {
                                if fp_id == peer_id {
                                    continue;
                                }
                                let Some(fp) = shared.peer_manager.get(fp_id) else {
                                    continue;
                                };
                                if !fp.is_sync_eligible() {
                                    continue;
                                }
                                if !router.should_relay_to(hash, origin_device, fp_id, &relay_path)
                                {
                                    continue;
                                }
                                let mut extended_path = relay_path.clone();
                                extended_path.push(shared.config.device_name.clone());
                                let _ = fp_tx.try_send(AppMessage::ClipboardPush {
                                    seq,
                                    content: content.clone(),
                                    origin_device,
                                    origin_device_name: display_name.clone(),
                                    relay_path: extended_path,
                                });
                            }
                        } else {
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::Warning(format!(
                                    "ignoring clipboard payload from untrusted/paused peer {}",
                                    peer_name
                                )))
                                .await;
                        }
                    }
                    Ok(AppMessage::FileTransferAnnounce { meta }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            let _ = rx_session_outbox_tx
                                .send(AppMessage::FileTransferCancel {
                                    transfer_id: meta.transfer_id,
                                    reason: "Device not trusted (Accept pairing request first)"
                                        .to_string(),
                                })
                                .await;
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::Warning(format!(
                                    "ignoring file transfer from untrusted peer {}",
                                    peer_name
                                )))
                                .await;
                            continue;
                        }
                        let transfer_id = meta.transfer_id;
                        let file_name = meta.file_name.clone();
                        let file_bytes = meta.size_bytes;
                        let mime_type = meta.mime_type.clone();

                        // Register inbound transfer.
                        let reg_result = shared
                            .file_transfers
                            .lock()
                            .await
                            .register_inbound(meta, peer_id, peer_name.clone())
                            .map(|t| t.dest_path.is_some());
                        // Already accepted once: this is the sender resuming
                        // after a reconnect, so continue without asking again.
                        let resuming = matches!(reg_result, Ok(true));
                        if let Err(e) = reg_result {
                            tracing::warn!(error = %e, "rejected file transfer announce");
                            let _ = rx_session_outbox_tx
                                .send(AppMessage::FileTransferCancel {
                                    transfer_id,
                                    reason: e.to_string(),
                                })
                                .await;
                            continue;
                        }

                        // Check auto-accept policy: trusted/paired devices auto-accept without requiring manual approval.
                        let is_trusted = shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false);
                        let settings = shared.settings.lock().unwrap().clone();
                        let auto_accept = (is_trusted || settings.auto_accept_file_transfers)
                            && (settings.auto_accept_max_bytes == 0
                                || file_bytes <= settings.auto_accept_max_bytes);

                        if resuming {
                            let _ = shared
                                .file_transfers
                                .lock()
                                .await
                                .queue_inbound(&transfer_id);
                            let bg_shared = shared.clone();
                            tokio::spawn(async move {
                                pump_transfer_queue(&bg_shared).await;
                            });
                        } else if auto_accept {
                            let _ = shared
                                .file_transfers
                                .lock()
                                .await
                                .queue_inbound(&transfer_id);

                            // Trigger the queue manager
                            let bg_shared = shared.clone();
                            tokio::spawn(async move {
                                pump_transfer_queue(&bg_shared).await;
                            });

                            // Record in feed.
                            shared.activity.lock().await.record_file_transfer_started(
                                peer_id,
                                peer_name.clone(),
                                file_name.clone(),
                                file_bytes,
                                hex::encode(transfer_id),
                                false,
                            );
                        } else {
                            // Prompt the user via event.
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::FileTransferIncoming {
                                    transfer_id,
                                    from_device: peer_id,
                                    from_name: peer_name.clone(),
                                    file_name,
                                    file_bytes,
                                    mime_type,
                                })
                                .await;
                        }
                    }
                    Ok(AppMessage::FileTransferAccept {
                        transfer_id,
                        accepted,
                        resume_from_chunk,
                        reject_reason,
                    }) => {
                        touch_last_seen();
                        if !accepted {
                            shared
                                .file_transfers
                                .lock()
                                .await
                                .cancel_outbound(&transfer_id);
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::FileTransferFailed {
                                    transfer_id,
                                    from_device: peer_id,
                                    reason: reject_reason.unwrap_or_else(|| "rejected".into()),
                                })
                                .await;
                        } else {
                            {
                                let mut mgr = shared.file_transfers.lock().await;
                                if let Some(transfer) = mgr.get_outbound_mut(&transfer_id) {
                                    transfer.resume_from(resume_from_chunk);
                                }
                            }
                            let bg_outbox = shared
                                .peer_manager
                                .file_sender(peer_id)
                                .unwrap_or(session_outbox_tx.clone());
                            let bg_shared = shared.clone();
                            let bg_event_tx = shared.event_tx.clone();
                            let bg_transfer_id = transfer_id;
                            let bg_peer_id = peer_id;
                            let mut bg_last_prog_emit: std::collections::HashMap<
                                [u8; 16],
                                std::time::Instant,
                            > = std::collections::HashMap::new();
                            tokio::spawn(async move {
                                const BATCH_SIZE: usize = 4;
                                'outer: loop {
                                    let (next_chunk, _last_acked, total_chunks): (u32, u32, u32) = {
                                        let mut mgr = bg_shared.file_transfers.lock().await;
                                        if let Some(t) = mgr.get_outbound_mut(&bg_transfer_id) {
                                            (
                                                t.next_chunk,
                                                t.last_acked_chunk.unwrap_or(0),
                                                t.total_chunks,
                                            )
                                        } else {
                                            break 'outer;
                                        }
                                    };
                                    if next_chunk >= total_chunks {
                                        break 'outer;
                                    }

                                    let (batch, progs) = match read_outbound_chunks(
                                        bg_shared.clone(),
                                        bg_transfer_id,
                                        BATCH_SIZE,
                                    )
                                    .await
                                    {
                                        Some((batch, progs)) => (batch, progs),
                                        None => break 'outer,
                                    };

                                    if let Some((prog, fname)) = progs.last() {
                                        let now = std::time::Instant::now();
                                        let last = bg_last_prog_emit
                                            .get(&bg_transfer_id)
                                            .copied()
                                            .unwrap_or_else(|| {
                                                now.checked_sub(std::time::Duration::from_secs(1))
                                                    .unwrap()
                                            });
                                        if now.duration_since(last).as_millis() >= 100
                                            || prog.percent == 100
                                        {
                                            bg_last_prog_emit.insert(bg_transfer_id, now);
                                            let _ = bg_event_tx.try_send(
                                                EngineEvent::FileTransferProgress {
                                                    transfer_id: bg_transfer_id,
                                                    from_device: bg_peer_id,
                                                    file_name: fname.clone(),
                                                    percent: prog.percent,
                                                    bytes_received: prog.bytes_received,
                                                    total_bytes: prog.total_bytes,
                                                    speed_bps: prog.speed_bps,
                                                    eta_secs: prog.eta_secs,
                                                },
                                            );
                                        }
                                    }

                                    if batch.is_empty() {
                                        break;
                                    }
                                    for wire_msg in batch {
                                        if bg_outbox.send(wire_msg).await.is_err() {
                                            break 'outer;
                                        }
                                    }
                                }

                                let final_checksum = {
                                    let mut mgr = bg_shared.file_transfers.lock().await;
                                    mgr.get_outbound_mut(&bg_transfer_id).and_then(|transfer| {
                                        if transfer.is_all_sent() {
                                            Some(transfer.finalize_checksum())
                                        } else {
                                            None
                                        }
                                    })
                                };
                                if let Some(sha256_checksum) = final_checksum {
                                    let _ = bg_outbox
                                        .send(AppMessage::FileTransferComplete {
                                            transfer_id: bg_transfer_id,
                                            sha256_checksum,
                                        })
                                        .await;
                                }
                            });
                        }
                    }
                    Ok(AppMessage::FileChunk {
                        transfer_id,
                        chunk_index,
                        total_chunks: _,
                        data: payload,
                        compressed,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }

                        let data = if compressed {
                            match lz4_flex::decompress_size_prepended(&payload) {
                                Ok(d) => d,
                                Err(e) => {
                                    tracing::error!("Failed to decompress file chunk: {}", e);
                                    let mut mgr = shared.file_transfers.lock().await;
                                    mgr.cancel_inbound(&transfer_id, "decompression failed");
                                    continue;
                                }
                            }
                        } else {
                            payload
                        };

                        let validation = {
                            let mut mgr = shared.file_transfers.lock().await;
                            if let Some(transfer) = mgr.get_inbound_mut(&transfer_id) {
                                if transfer.from_device != peer_id {
                                    Err(anyhow::anyhow!("peer mismatch"))
                                } else {
                                    transfer.validate_chunk(chunk_index, data.len())
                                }
                            } else {
                                Err(anyhow::anyhow!("unknown transfer"))
                            }
                        };

                        match validation {
                            Ok((offset, padding, is_duplicate)) => {
                                if is_duplicate {
                                    let mut mgr = shared.file_transfers.lock().await;
                                    if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                                        let prog = t.progress_snapshot();
                                        let should_ack = t.should_ack();
                                        let file_name = t.meta.file_name.clone();
                                        let last_confirmed = t.last_confirmed_chunk;
                                        drop(mgr);

                                        let _ = shared.event_tx.try_send(
                                            EngineEvent::FileTransferProgress {
                                                transfer_id,
                                                from_device: peer_id,
                                                file_name,
                                                percent: prog.percent,
                                                bytes_received: prog.bytes_received,
                                                total_bytes: prog.total_bytes,
                                                speed_bps: prog.speed_bps,
                                                eta_secs: prog.eta_secs,
                                            },
                                        );
                                        if should_ack {
                                            let _ = rx_session_outbox_tx
                                                .send(AppMessage::FileChunkAck {
                                                    transfer_id,
                                                    last_confirmed_chunk: last_confirmed,
                                                })
                                                .await;
                                        }
                                    }
                                } else {
                                    let _ = disk_tx
                                        .send(DiskTaskMsg::Chunk {
                                            transfer_id,
                                            chunk_index,
                                            offset,
                                            padding,
                                            data,
                                        })
                                        .await;
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to validate chunk: {:?}", e);
                            }
                        };
                    }
                    Ok(AppMessage::FileChunkAck {
                        transfer_id,
                        last_confirmed_chunk,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        if let Some(transfer) = shared
                            .file_transfers
                            .lock()
                            .await
                            .get_outbound_mut(&transfer_id)
                        {
                            if transfer.target_device == Some(peer_id)
                                || transfer.target_device.is_none()
                            {
                                transfer.on_chunk_ack(last_confirmed_chunk);
                                let prog = transfer.progress();
                                let fname = transfer.meta.file_name.clone();
                                let event_tx = shared.event_tx.clone();
                                let tid = transfer_id;
                                tokio::spawn(async move {
                                    let _ = event_tx.try_send(EngineEvent::FileTransferProgress {
                                        transfer_id: tid,
                                        from_device: peer_id,
                                        file_name: fname,
                                        percent: prog.percent,
                                        bytes_received: prog.bytes_received,
                                        total_bytes: prog.total_bytes,
                                        speed_bps: prog.speed_bps,
                                        eta_secs: prog.eta_secs,
                                    });
                                });
                            }
                        }
                    }
                    Ok(AppMessage::FileTransferComplete {
                        transfer_id,
                        sha256_checksum,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        // Finalize: verify SHA-256 and write to disk.
                        let _ = rx_disk_tx
                            .send(DiskTaskMsg::Complete {
                                transfer_id,
                                sha256_checksum,
                            })
                            .await;
                    }
                    Ok(AppMessage::FileTransferCompleteAck {
                        transfer_id,
                        success,
                        error,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }

                        let (file_name, peer_name) = {
                            let mut mgr = shared.file_transfers.lock().await;
                            let fname = mgr
                                .get_outbound_mut(&transfer_id)
                                .map(|t| t.meta.file_name.clone())
                                .unwrap_or_default();
                            mgr.remove_outbound(&transfer_id);
                            let pname = shared
                                .peer_manager
                                .get(peer_id)
                                .map(|p| p.friendly_name.clone())
                                .unwrap_or_default();
                            (fname, pname)
                        };

                        if success {
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::FileTransferComplete {
                                    transfer_id,
                                    from_device: peer_id,
                                    from_name: peer_name,
                                    file_name,
                                    dest_path: std::path::PathBuf::new(),
                                })
                                .await;
                        } else {
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::FileTransferFailed {
                                    transfer_id,
                                    from_device: peer_id,
                                    reason: error.unwrap_or_else(|| "Unknown error".to_string()),
                                })
                                .await;
                        }
                    }
                    Ok(AppMessage::FileTransferCancel {
                        transfer_id,
                        reason,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        {
                            let mut mgr = shared.file_transfers.lock().await;
                            if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                                if t.from_device == peer_id {
                                    mgr.cancel_inbound(&transfer_id, &reason);
                                }
                            }
                            if let Some(t) = mgr.get_outbound_mut(&transfer_id) {
                                if t.target_device == Some(peer_id) || t.target_device.is_none() {
                                    mgr.cancel_outbound(&transfer_id);
                                }
                            }
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::FileTransferFailed {
                                transfer_id,
                                from_device: peer_id,
                                reason,
                            })
                            .await;
                        pump_transfer_queue(&shared).await;
                    }
                    Ok(AppMessage::FileTransferPause { transfer_id }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        {
                            let mut mgr = shared.file_transfers.lock().await;
                            if let Some(t) = mgr.get_outbound_mut(&transfer_id) {
                                if t.target_device == Some(peer_id) || t.target_device.is_none() {
                                    t.paused = true;
                                }
                            } else if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                                if t.from_device == peer_id {
                                    t.paused = true;
                                }
                            }
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::FileTransferPaused { transfer_id })
                            .await;
                    }
                    Ok(AppMessage::FileTransferResume { transfer_id }) => {
                        touch_last_seen();
                        let mut was_outbound = false;
                        {
                            let mut mgr = shared.file_transfers.lock().await;
                            if let Some(t) = mgr.get_outbound_mut(&transfer_id) {
                                if t.target_device == Some(peer_id) || t.target_device.is_none() {
                                    t.paused = false;
                                    was_outbound = true;
                                }
                            } else if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                                if t.from_device == peer_id {
                                    t.paused = false;
                                }
                            }
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::FileTransferResumed { transfer_id })
                            .await;

                        if was_outbound {
                            // Resume the background chunk loop if we are the sender
                            let bg_outbox = shared
                                .peer_manager
                                .file_sender(peer_id)
                                .unwrap_or(session_outbox_tx.clone());
                            let bg_shared = shared.clone();
                            let bg_transfer_id = transfer_id;
                            let bg_event_tx = shared.event_tx.clone();
                            let bg_peer_id = peer_id;
                            let mut bg_last_prog_emit: std::collections::HashMap<
                                [u8; 16],
                                std::time::Instant,
                            > = std::collections::HashMap::new();
                            tokio::spawn(async move {
                                const BATCH_SIZE: usize = 4;
                                'outer: loop {
                                    let (next_chunk, _last_acked, total_chunks): (u32, u32, u32) = {
                                        let mut mgr = bg_shared.file_transfers.lock().await;
                                        if let Some(t) = mgr.get_outbound_mut(&bg_transfer_id) {
                                            (
                                                t.next_chunk,
                                                t.last_acked_chunk.unwrap_or(0),
                                                t.total_chunks,
                                            )
                                        } else {
                                            break 'outer;
                                        }
                                    };
                                    if next_chunk >= total_chunks {
                                        break 'outer;
                                    }

                                    let (batch, progs) = match read_outbound_chunks(
                                        bg_shared.clone(),
                                        bg_transfer_id,
                                        BATCH_SIZE,
                                    )
                                    .await
                                    {
                                        Some((batch, progs)) => (batch, progs),
                                        None => break 'outer,
                                    };

                                    if let Some((prog, fname)) = progs.last() {
                                        let now = std::time::Instant::now();
                                        let last = bg_last_prog_emit
                                            .get(&bg_transfer_id)
                                            .copied()
                                            .unwrap_or_else(|| {
                                                now.checked_sub(std::time::Duration::from_secs(1))
                                                    .unwrap()
                                            });
                                        if now.duration_since(last).as_millis() >= 100
                                            || prog.percent == 100
                                        {
                                            bg_last_prog_emit.insert(bg_transfer_id, now);
                                            let _ = bg_event_tx.try_send(
                                                EngineEvent::FileTransferProgress {
                                                    transfer_id: bg_transfer_id,
                                                    from_device: bg_peer_id,
                                                    file_name: fname.clone(),
                                                    percent: prog.percent,
                                                    bytes_received: prog.bytes_received,
                                                    total_bytes: prog.total_bytes,
                                                    speed_bps: prog.speed_bps,
                                                    eta_secs: prog.eta_secs,
                                                },
                                            );
                                        }
                                    }

                                    if batch.is_empty() {
                                        break 'outer;
                                    }

                                    // Send the batch
                                    for wire_msg in batch {
                                        if bg_outbox.send(wire_msg).await.is_err() {
                                            break 'outer;
                                        }
                                    }
                                }

                                let final_checksum = {
                                    let mut mgr = bg_shared.file_transfers.lock().await;
                                    mgr.get_outbound_mut(&bg_transfer_id).and_then(|transfer| {
                                        if transfer.is_all_sent() {
                                            Some(transfer.finalize_checksum())
                                        } else {
                                            None
                                        }
                                    })
                                };
                                if let Some(sha256_checksum) = final_checksum {
                                    let _ = bg_outbox
                                        .send(AppMessage::FileTransferComplete {
                                            transfer_id: bg_transfer_id,
                                            sha256_checksum,
                                        })
                                        .await;
                                }
                            });
                        }
                    }
                    Ok(AppMessage::SpeedTestRequest {
                        test_id,
                        duration_secs,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }

                        let mut can_accept = false;
                        {
                            let mut tests = shared.speed_tests.lock().await;
                            // Accept if we aren't already running a test with this peer
                            let entry = tests.entry(peer_id).or_insert_with(|| {
                                crate::speed_test::SpeedTestState::new(session_outbox_tx.clone())
                            });
                            if entry.phase == crate::speed_test::SpeedTestPhase::Idle {
                                entry.start_receiving(test_id, duration_secs);
                                can_accept = true;
                            }
                        }

                        let _ = session_outbox_tx
                            .send(AppMessage::SpeedTestResponse {
                                test_id,
                                accepted: can_accept,
                                reason: if can_accept {
                                    None
                                } else {
                                    Some("Busy".into())
                                },
                            })
                            .await;
                    }
                    Ok(AppMessage::SpeedTestResponse {
                        test_id,
                        accepted,
                        reason: _,
                    }) => {
                        touch_last_seen();
                        if accepted {
                            let mut tests = shared.speed_tests.lock().await;
                            if let Some(state) = tests.get_mut(&peer_id) {
                                if state.test_id == Some(test_id) {
                                    state.start_sending(test_id, state.duration_secs);
                                }
                            }
                        } else {
                            let mut tests = shared.speed_tests.lock().await;
                            if let Some(state) = tests.get_mut(&peer_id) {
                                if state.test_id == Some(test_id) {
                                    state.reset();
                                }
                            }
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::SpeedTestComplete { test_id, peer_id })
                                .await;
                        }
                    }
                    Ok(AppMessage::SpeedTestData {
                        test_id,
                        seq: _,
                        data,
                    }) => {
                        touch_last_seen();
                        let send_stats = {
                            let mut tests = shared.speed_tests.lock().await;
                            if let Some(state) = tests.get_mut(&peer_id) {
                                if state.test_id == Some(test_id)
                                    && state.phase == crate::speed_test::SpeedTestPhase::Receiving
                                {
                                    state.handle_chunk(data.len());

                                    // Should we emit stats back to sender?
                                    if let Some(last_tick) = state.last_tick_time {
                                        if last_tick.elapsed().as_millis() >= 500 {
                                            state.last_tick_time = Some(std::time::Instant::now());
                                            Some(
                                                state
                                                    .bytes_transferred
                                                    .load(std::sync::atomic::Ordering::Relaxed),
                                            )
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        };

                        if let Some(bytes) = send_stats {
                            let _ = session_outbox_tx
                                .send(AppMessage::SpeedTestStats {
                                    test_id,
                                    received_bytes: bytes,
                                })
                                .await;

                            let duration_secs = {
                                let tests = shared.speed_tests.lock().await;
                                tests.get(&peer_id).map(|s| s.duration_secs)
                            };
                            if let Some(dur) = duration_secs {
                                let _ = shared.event_tx.try_send(EngineEvent::SpeedTestProgress {
                                    test_id,
                                    peer_id,
                                    direction: "download".to_string(),
                                    bytes_transferred: bytes,
                                    duration_secs: dur,
                                });
                            }
                        }
                    }
                    Ok(AppMessage::SpeedTestStats {
                        test_id,
                        received_bytes,
                    }) => {
                        touch_last_seen();
                        let duration_secs = {
                            let tests = shared.speed_tests.lock().await;
                            if let Some(state) = tests.get(&peer_id) {
                                if state.test_id == Some(test_id) {
                                    Some(state.duration_secs)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        };

                        if let Some(dur) = duration_secs {
                            let _ = shared.event_tx.try_send(EngineEvent::SpeedTestProgress {
                                test_id,
                                peer_id,
                                direction: "upload".to_string(),
                                bytes_transferred: received_bytes,
                                duration_secs: dur,
                            });
                        }
                    }
                    Ok(AppMessage::SpeedTestComplete { test_id }) => {
                        touch_last_seen();
                        let mut tests = shared.speed_tests.lock().await;
                        if let Some(state) = tests.get_mut(&peer_id) {
                            if state.test_id == Some(test_id) {
                                state.reset();
                            }
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::SpeedTestComplete { test_id, peer_id })
                            .await;
                    }
                    Ok(AppMessage::HistoryMetadata { entry }) => {
                        touch_last_seen();
                        // MED-03 FIX: Only accept history metadata from trusted peers.
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let _ = shared.peer_manager.update_last_sync(peer_id);
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::HistoryMetadataReceived {
                                from_device: peer_id,
                                from_name: peer_name.clone(),
                                entry,
                            })
                            .await;
                    }
                    Ok(AppMessage::DeviceSleepState { is_asleep }) => {
                        touch_last_seen();
                        tracing::info!(peer = %peer_name, is_asleep, "received device sleep state");
                        rx_peer_sleeping.store(is_asleep, std::sync::atomic::Ordering::Relaxed);
                        // Send an instant ping if they just woke up to update their last_seen_millis
                        // and prevent their local 15s grace period from expiring before our next tick.
                        if !is_asleep {
                            let ping = probe::make_ping();
                            *rx_ping_sent_at.lock().unwrap_or_else(|e| e.into_inner()) =
                                Some(std::time::Instant::now());
                            let _ = rx_session_outbox_tx.send(ping).await;
                        }
                    }
                    Ok(AppMessage::DeviceSyncState { enabled }) => {
                        touch_last_seen();
                        tracing::info!(peer = %peer_name, enabled, "received device sync state");
                        let _ = shared
                            .peer_manager
                            .set_remote_sync_enabled(peer_id, enabled);
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::PeerSyncStateChanged {
                                device_id: peer_id,
                                enabled,
                            })
                            .await;
                    }
                    Ok(AppMessage::ClipboardAck { seq }) => {
                        touch_last_seen();
                        let _ = shared.peer_manager.update_last_sync(peer_id);
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::ClipboardSynced {
                                peer_device: peer_id,
                                peer_name: peer_name.clone(),
                                seq,
                            })
                            .await;
                    }
                    Ok(AppMessage::KeyRotated { new_pubkey_bytes }) => {
                        touch_last_seen();
                        // Only accept key rotation from currently trusted peers over the established AEAD tunnel.
                        if shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            let mut trust = shared.trust.lock().await;
                            if trust.rotate_peer_key(peer_id, &new_pubkey_bytes).is_ok() {
                                tracing::info!(peer_id = %peer_id, "Successfully processed KeyRotated from peer");
                            }
                        }
                    }
                    Ok(AppMessage::Ping { timestamp_ms }) => {
                        touch_last_seen();
                        let _ = rx_session_outbox_tx
                            .send(AppMessage::Pong { timestamp_ms })
                            .await;
                    }
                    Ok(AppMessage::PairingRequest {
                        origin_device,
                        origin_device_name,
                        pin: _req_pin,
                    }) => {
                        touch_last_seen();

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
                            let _ = rx_session_outbox_tx
                                .send(AppMessage::PairingResponse {
                                    origin_device: shared.config.device_id,
                                    accepted: true,
                                })
                                .await;
                            continue;
                        }

                        let _ = shared.peer_manager.set_pairing_requested(peer_id, true);

                        // Re-emit PairingRequested with the REAL name and PIN so the UI updates
                        let pin = rx_session_pin
                            .clone()
                            .or_else(|| {
                                shared.peer_manager.get(peer_id).and_then(|p| p.pairing_pin)
                            })
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
                    Ok(AppMessage::PairingResponse {
                        origin_device,
                        accepted,
                    }) => {
                        touch_last_seen();

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
                            continue;
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
                            continue;
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
                            break "peer rejected pairing request".to_string();
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
                                    addr: endpoint,
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
                    Ok(AppMessage::QrAuth { token }) => {
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
                                    addr: endpoint,
                                    trusted: true,
                                })
                                .await;
                        } else {
                            tracing::warn!(peer_id = %peer_id, "peer provided invalid QR auth token");
                        }
                    }
                    Ok(AppMessage::Pong { timestamp_ms: _ }) => {
                        touch_last_seen();
                        // Feed the RTT sample into the peer's quality probe
                        // using the Instant captured at send time, which is
                        // far more accurate than round-tripping wall-clock ms
                        // over the network (HIGH-03).
                        let maybe_sent_at = rx_ping_sent_at
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .take();
                        if let Some(sent_at) = maybe_sent_at {
                            let rtt_us = probe::measure_rtt_us(sent_at);
                            let result = ProbeResult::from_samples(vec![rtt_us]);

                            shared
                                .quality_probes
                                .entry(peer_id)
                                .or_insert_with(|| QualityProbe::new(peer_name.as_str()))
                                .record(result);
                        }
                    }
                    Ok(AppMessage::CallStateUpdate {
                        state,
                        number,
                        contact_name,
                        origin_device,
                        origin_device_name,
                    }) => {
                        touch_last_seen();
                        // MED-03 FIX: Only process call state from trusted peers.
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        // Persist in shared state for IPC status polling.
                        {
                            let mut call = shared.device_status.active_call.lock().await;
                            if state == "idle" {
                                *call = None;
                            } else {
                                *call = Some(ActiveCallState {
                                    device_id: origin_device,
                                    device_name: origin_device_name.clone(),
                                    state: state.clone(),
                                    number: number.clone(),
                                    contact_name: contact_name.clone(),
                                });
                            }
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::CallStateChanged {
                                from_device: origin_device,
                                from_name: origin_device_name,
                                state,
                                number,
                                contact_name,
                            })
                            .await;
                    }
                    Ok(AppMessage::BatteryStatus {
                        level,
                        charging,
                        origin_device,
                        origin_device_name,
                    }) => {
                        touch_last_seen();
                        tracing::info!(
                            "Received BatteryStatus: level={}, charging={} from {}",
                            level,
                            charging,
                            origin_device
                        );
                        // MED-03 FIX: Only process battery status from trusted peers.
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            tracing::warn!(
                                "Ignoring BatteryStatus from untrusted peer {}",
                                peer_id
                            );
                            continue;
                        }
                        // Persist in shared state for IPC status polling.
                        {
                            shared.device_status.peer_batteries.insert(
                                origin_device,
                                PeerBatteryState {
                                    device_id: origin_device,
                                    device_name: origin_device_name.clone(),
                                    level,
                                    charging,
                                },
                            );
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::BatteryStateChanged {
                                from_device: origin_device,
                                from_name: origin_device_name,
                                level,
                                charging,
                            })
                            .await;
                    }
                    Ok(AppMessage::NetworkStatus {
                        network_type,
                        origin_device,
                        origin_device_name,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        {
                            shared.device_status.peer_networks.insert(
                                origin_device,
                                PeerNetworkState {
                                    device_id: origin_device,
                                    device_name: origin_device_name.clone(),
                                    network_type: network_type.clone(),
                                },
                            );
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::NetworkStateChanged {
                                from_device: origin_device,
                                from_name: origin_device_name,
                                network_type,
                            })
                            .await;
                    }
                    Ok(AppMessage::StorageStatus {
                        images_bytes,
                        videos_bytes,
                        apps_bytes,
                        free_bytes,
                        total_bytes,
                        origin_device,
                        origin_device_name,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        {
                            shared.device_status.peer_storage.insert(
                                origin_device,
                                PeerStorageState {
                                    device_id: origin_device,
                                    device_name: origin_device_name.clone(),
                                    images_bytes,
                                    videos_bytes,
                                    apps_bytes,
                                    free_bytes,
                                    total_bytes,
                                },
                            );
                        }
                    }
                    Ok(AppMessage::CallAction {
                        action,
                        origin_device,
                    }) => {
                        touch_last_seen();
                        if action == "system:explicit_disconnect" {
                            tracing::info!("Peer explicitly disconnected. Pausing auto-reconnect.");
                            let _ = shared.peer_manager.set_explicit_disconnect(peer_id, true);
                            break "explicitly disconnected by peer".to_string();
                        }
                        tracing::info!("Received CallAction: {} from {:?}", action, origin_device);
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::CallActionRequest {
                                action,
                                from_device: origin_device,
                            })
                            .await;
                    }
                    Ok(AppMessage::Bye) => {
                        // Do NOT set explicit_disconnect here — receiving Bye from
                        // the remote peer (e.g., Android OS killed the socket) is
                        // not a user-initiated disconnect. Auto-reconnect must stay
                        // enabled so the watchdog can re-establish the link.
                        break "peer closed session".to_string();
                    }
                    Ok(AppMessage::PermissionError {
                        feature,
                        message,
                        origin_device: _,
                        origin_device_name: _,
                    }) => {
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::Warning(format!("{}: {}", feature, message)))
                            .await;
                    }
                    Ok(AppMessage::NotificationRelay {
                        id,
                        package,
                        title,
                        text,
                        origin_device,
                        origin_device_name,
                    }) => {
                        touch_last_seen();
                        // MED-03 FIX: Only process notifications from trusted peers.
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let _activity_id = {
                            let mut feed = shared.activity.lock().await;
                            feed.record_remote_notification(
                                origin_device,
                                origin_device_name.clone(),
                                package.clone(),
                                title.clone(),
                                text.clone(),
                            )
                        };
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::NotificationReceived {
                                id,
                                package,
                                title,
                                text,
                                from_device: origin_device,
                                from_name: origin_device_name,
                            })
                            .await;
                    }
                    Ok(AppMessage::CameraStreamRequest { origin_device }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::CameraStreamRequest {
                                from_device: origin_device,
                            })
                            .await;
                    }
                    Ok(AppMessage::CameraStreamAccept {
                        origin_device,
                        accepted,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::CameraStreamAccept {
                                from_device: origin_device,
                                accepted,
                            })
                            .await;
                    }
                    Ok(AppMessage::CameraStreamStop { origin_device }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        shared.camera_frames.remove(&origin_device);
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::CameraStreamStop {
                                from_device: origin_device,
                            })
                            .await;
                    }
                    Ok(AppMessage::CameraFrame {
                        origin_device,
                        data,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        shared.camera_frames.insert(origin_device, data);
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::CameraFrameReceived {
                                from_device: origin_device,
                            })
                            .await;
                    }
                    Ok(AppMessage::RemoteFilesQuery {
                        request_id,
                        origin_device,
                        summary_only,
                        category,
                        source,
                        search_query,
                        offset,
                        limit,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::RemoteFilesQueryReceived {
                                request_id,
                                from_device: origin_device,
                                summary_only,
                                category,
                                source,
                                search_query,
                                offset,
                                limit,
                            })
                            .await;
                    }
                    Ok(AppMessage::RemoteFilesResponse {
                        request_id,
                        summary,
                        files,
                        total_matching,
                        error,
                    }) => {
                        touch_last_seen();
                        if let Some((_target, tx)) =
                            shared.remote_waiters.files.lock().await.remove(&request_id)
                        {
                            let _ = tx.send(RemoteFilesResult {
                                summary: summary.clone(),
                                files: files.clone(),
                                total_matching,
                                error: error.clone(),
                            });
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::RemoteFilesResponseReceived {
                                request_id,
                                from_device: peer_id,
                                summary,
                                files,
                                total_matching,
                                error,
                            })
                            .await;
                    }
                    Ok(AppMessage::RemoteThumbnailRequest {
                        request_id,
                        origin_device,
                        file_id,
                        size_px,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::RemoteThumbnailRequestReceived {
                                request_id,
                                from_device: origin_device,
                                file_id,
                                size_px,
                            })
                            .await;
                    }
                    Ok(AppMessage::RemoteThumbnailResponse {
                        request_id,
                        file_id,
                        data,
                        error,
                    }) => {
                        touch_last_seen();
                        if let Some((_target, tx)) = shared
                            .remote_waiters
                            .thumbnails
                            .lock()
                            .await
                            .remove(&request_id)
                        {
                            let _ = tx.send(RemoteThumbnailResult {
                                file_id,
                                data: data.clone(),
                                error: error.clone(),
                            });
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::RemoteThumbnailResponseReceived {
                                request_id,
                                from_device: peer_id,
                                file_id,
                                data,
                                error,
                            })
                            .await;
                    }
                    Ok(AppMessage::RemoteFilePullRequest {
                        request_id,
                        origin_device,
                        file_id,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::RemoteFilePullRequestReceived {
                                request_id,
                                from_device: origin_device,
                                file_id,
                            })
                            .await;
                    }
                    Ok(AppMessage::RemoteFileActionRequest {
                        action,
                        file_id,
                        new_name,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::RemoteFileActionRequestReceived {
                                from_device: peer_id,
                                action,
                                file_id,
                                new_name,
                            })
                            .await;
                    }
                    Ok(AppMessage::OpenUrlOnDevice {
                        url,
                        origin_device: _,
                        origin_device_name: _,
                    }) => {
                        touch_last_seen();
                        if !shared
                            .peer_manager
                            .get(peer_id)
                            .map(|p| p.trusted)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        // Restrict to http/https — this is the only case
                        // actually asked for ("open a link"), and it closes
                        // off URI-scheme-hijacking as a risk class entirely.
                        if url.starts_with("http://") || url.starts_with("https://") {
                            let _ = shared
                                .event_tx
                                .send(EngineEvent::OpenUrlOnDeviceRequested {
                                    from_device: peer_id,
                                    from_name: peer_name.clone(),
                                    url,
                                })
                                .await;
                        } else {
                            let _ = rx_session_outbox_tx
                                .send(AppMessage::OpenUrlOnDeviceAck {
                                    success: false,
                                    error: Some(
                                        "rejected: only http/https URLs are allowed".into(),
                                    ),
                                })
                                .await;
                        }
                    }
                    Ok(AppMessage::OpenUrlOnDeviceAck { success, error }) => {
                        touch_last_seen();
                        let _ = shared
                            .event_tx
                            .send(EngineEvent::OpenUrlOnDeviceAckReceived {
                                from_device: peer_id,
                                success,
                                error,
                            })
                            .await;
                    }
                    Ok(AppMessage::Hello { .. }) | Ok(AppMessage::HelloAck { .. }) => {
                        // Ignored in session loop
                    }
                    Err(err) => {
                        break err.to_string();
                    }
                }
            }
        });

        let mut last_tick_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let disconnect_reason = loop {
            let now_millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let last_seen_millis = last_seen.load(std::sync::atomic::Ordering::Relaxed);

            tokio::select! {
                biased;
                shutdown = &mut shutdown_rx => {
                    match shutdown {
                        Ok(cmd) => {
                            if cmd.explicit_disconnect {
                                let _ = sess_tx.send(&mut AppMessage::CallAction {
                                    action: "system:explicit_disconnect".to_string(),
                                    origin_device: shared.config.device_id,
                                }).await;
                            }
                            if cmd.send_bye {
                                let _ = sess_tx.send(&mut AppMessage::Bye).await;
                            }
                            break cmd.reason;
                        }
                        Err(_) => {
                            break "session shutdown channel dropped".to_string();
                        }
                    }
                }
                _ = heartbeat.tick() => {
                    let tick_delta = now_millis.saturating_sub(last_tick_millis);
                    last_tick_millis = now_millis;

                    // If this tokio interval tick took significantly longer than expected (e.g. > 20s),
                    // the OS almost certainly suspended our CPU (Doze mode or sleep).
                    // We implicitly grant ourselves a fresh 15-second grace period.
                    if tick_delta > (shared.config.heartbeat_interval.as_millis() as u64) + 5000 {
                        shared.local_last_wake.store(now_millis, std::sync::atomic::Ordering::Relaxed);
                    }

                    // Either side being asleep relaxes the heartbeat: the
                    // sleeping side pings rarely, and the awake side was told
                    // (DeviceSleepState) to do the same, so neither may time
                    // the other out on silence. Dead links still surface as
                    // TCP errors via keepalive.
                    let is_sleeping = peer_sleeping.load(std::sync::atomic::Ordering::Relaxed)
                        || shared.local_sleeping.load(std::sync::atomic::Ordering::Relaxed);
                    let timeout = if is_sleeping {
                        // 24 hours timeout if peer is sleeping
                        24 * 60 * 60 * 1000
                    } else {
                        shared.config.heartbeat_timeout.as_millis() as u64
                    };

                    let time_since_seen = now_millis.saturating_sub(last_seen_millis);
                    let time_since_wake = now_millis.saturating_sub(shared.local_last_wake.load(std::sync::atomic::Ordering::Relaxed));

                    // Only timeout if we haven't seen a heartbeat in `timeout` ms AND
                    // we've been awake for at least `timeout` ms.
                    // This prevents us from disconnecting immediately when our own CPU wakes from deep sleep.
                    if time_since_seen > timeout && time_since_wake > timeout {
                        break format!("heartbeat timeout (sleeping: {is_sleeping}, time_since_seen: {time_since_seen}, time_since_wake: {time_since_wake})");
                    }

                    shared.file_transfers.lock().await.prune_stale_transfers();

                    // Only send a ping if awake, OR if asleep and we haven't seen them for 5 minutes
                    let should_ping = if !is_sleeping {
                        true
                    } else {
                        let last_ping_elapsed = ping_sent_at.lock().unwrap_or_else(|e| e.into_inner()).map(|i| i.elapsed().as_millis() as u64).unwrap_or(u64::MAX);
                        last_ping_elapsed > 5 * 60 * 1000
                    };

                    if should_ping {
                        let mut ping = probe::make_ping();
                        *ping_sent_at.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::time::Instant::now());
                        if let Err(err) = sess_tx.send(&mut ping).await {
                            break format!("heartbeat send failed: {err}");
                        }
                    }
                }
                Some(mut msg) = outbox_rx.recv() => {
                    if let Err(err) = sess_tx.send(&mut msg).await {
                        break format!("send failed: {err}");
                    }
                }
                Some(mut msg) = file_outbox_rx.recv() => {
                    if let Err(err) = sess_tx.send_no_flush(&mut msg).await {
                        break format!("send failed: {err}");
                    }
                    // Write a few queued chunks per flush, then go back to the
                    // select so pings, acks and clipboard never wait behind
                    // a long run of file data.
                    for _ in 0..3 {
                        match file_outbox_rx.try_recv() {
                            Ok(mut next_msg) => {
                                if let Err(_err) = sess_tx.send_no_flush(&mut next_msg).await {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    if let Err(err) = sess_tx.flush().await {
                        break format!("flush failed: {err}");
                    }
                }
                rx_res = &mut rx_task => {
                    match rx_res {
                        Ok(reason) => break reason,
                        Err(_) => break "rx task panicked".to_string(),
                    }
                }
            }
        };

        rx_task.abort();
        let reason = Some(disconnect_reason);
        match shared
            .peer_manager
            .mark_disconnected_if_current(peer_id, session_id, reason.clone())
        {
            Ok(Some(connected_at)) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let duration = now.saturating_sub(connected_at);
                if duration < 15 {
                    let _ = shared.event_tx.send(EngineEvent::Warning(
                        format!("Device '{}' disconnected rapidly ({}s). If this is an Android device, please ensure 'Ignore Battery Optimizations' (Background Execution) is enabled in its settings.", peer_name, duration)
                    )).await;
                }

                tracing::warn!(
                    "peer disconnected: peer_id={}, reason={:?}",
                    peer_id,
                    reason
                );
                let _ = shared
                    .event_tx
                    .send(EngineEvent::PeerDisconnected {
                        device_id: peer_id,
                        device_name: Some(peer_name.clone()),
                        reason: reason.clone(),
                    })
                    .await;

                shared.dedup.lock().await.remove_peer(peer_id);

                shared
                    .file_transfers
                    .lock()
                    .await
                    .pause_all_for_device(peer_id);
                shared.camera_frames.remove(&peer_id);
                pump_transfer_queue(&shared).await;

                // Drain pending remote file waiters and notify oneshot receivers with error fast-path
                drain_remote_waiters(&shared, peer_id).await;

                // FIX: Phantom Pairing Prompts. Clear incoming pairing state if connection drops.
                let _ = shared.peer_manager.set_pairing_requested(peer_id, false);
                let _ = shared.peer_manager.set_pairing_pin(peer_id, None);

                // Record in activity feed.
                let feed = shared.activity.clone();
                let name = peer_name.clone();
                let disc_reason = reason.clone();
                tokio::spawn(async move {
                    feed.lock()
                        .await
                        .record_peer_disconnected(peer_id, name, disc_reason);
                });

                if shared
                    .peer_manager
                    .get(peer_id)
                    .map(|peer| peer.should_auto_reconnect())
                    .unwrap_or(false)
                {
                    // ── AirDrop-style immediate reconnect ────────────────────
                    // Instead of waiting for the 3s auto-reconnector tick,
                    // spawn an immediate reconnect attempt after a short
                    // anti-loop delay. This cuts reconnect latency from ~3s
                    // to ~500ms for trusted peers.
                    let shared_reconnect = shared.clone();
                    let peer_endpoints = shared
                        .peer_manager
                        .get(peer_id)
                        .map(|p| p.socket_addrs())
                        .unwrap_or_default();
                    let peer_discovery = shared
                        .peer_manager
                        .get(peer_id)
                        .map(|p| p.discovery)
                        .unwrap_or(DiscoverySource::Unknown);
                    if !peer_endpoints.is_empty() {
                        tokio::spawn(async move {
                            // Small delay to prevent 0-delay infinite loops
                            // if the remote immediately resets.
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            // Only attempt if still disconnected (auto-reconnector
                            // may have already picked it up).
                            let still_offline = shared_reconnect
                                .peer_manager
                                .get(peer_id)
                                .map(|p| {
                                    p.status
                                        == crate::peer_manager::PeerConnectionState::Disconnected
                                        || p.status
                                            == crate::peer_manager::PeerConnectionState::Failed
                                })
                                .unwrap_or(false);
                            if still_offline {
                                tracing::debug!(
                                    peer_id = %peer_id,
                                    "immediate reconnect: attempting fast recovery"
                                );
                                let _ = connect_once(
                                    shared_reconnect,
                                    peer_endpoints,
                                    Some(peer_id),
                                    peer_discovery,
                                    false,
                                )
                                .await;
                            }
                        });
                    }
                }
            }
            Ok(None) => {
                drain_remote_waiters(&shared, peer_id).await;
            }
            Err(err) => {
                warn!(peer_id = %peer_id, error = %err, "failed to mark peer disconnected");
                drain_remote_waiters(&shared, peer_id).await;
            }
        }
    });

    // MED-02: observe the session task handle so panics surface as log errors
    // instead of being silently discarded by the Tokio runtime.
    tokio::spawn(async move {
        if let Err(panic) = session_handle.await {
            error!(
                peer_id = %peer_id,
                peer_name = %panic_peer_name,
                error = ?panic,
                "peer session task panicked — peer will appear disconnected"
            );
        }
    });

    Ok(())
}
