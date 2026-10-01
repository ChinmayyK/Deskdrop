//! Outbound and inbound file transfers: send, accept, reject, pause,
//! resume, cancel, and the transfer queue pump.

use super::*;

impl Engine {
    /// Send a file to a specific peer (or all if `target_device` is None).
    pub async fn send_file(
        &self,
        data: Vec<u8>,
        file_name: String,
        mime_type: String,
        target_device: Option<Uuid>,
    ) -> Result<[u8; 16]> {
        let mut mgr = self.shared.file_transfers.lock().await;
        let transfer = mgr.start_outbound(data, file_name.clone(), mime_type, target_device)?;
        let transfer_id = transfer.transfer_id;
        let meta = transfer.meta.clone();
        let size_bytes = meta.size_bytes;
        let _ = transfer;
        drop(mgr);

        self.announce_outbound_file_transfer(meta, file_name, size_bytes, target_device)
            .await?;
        Ok(transfer_id)
    }

    /// Send a file from disk without reading the full payload into memory first.
    pub async fn send_file_path(
        &self,
        path: PathBuf,
        file_name: String,
        mime_type: String,
        target_device: Option<Uuid>,
        batch_id: Option<String>,
        is_directory: bool,
        item_count: u32,
    ) -> Result<[u8; 16]> {
        self.send_outbound_source(
            path,
            None,
            file_name,
            mime_type,
            target_device,
            batch_id,
            is_directory,
            item_count,
        )
        .await
    }

    /// Send from an already-open file, so a caller that only has a handle
    /// (an Android content URI) need not copy the file somewhere first.
    pub async fn send_open_file(
        &self,
        file: std::fs::File,
        file_name: String,
        mime_type: String,
        target_device: Option<Uuid>,
    ) -> Result<[u8; 16]> {
        self.send_outbound_source(
            PathBuf::from(&file_name),
            Some(file),
            file_name,
            mime_type,
            target_device,
            None,
            false,
            1,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn send_outbound_source(
        &self,
        path: PathBuf,
        opened: Option<std::fs::File>,
        file_name: String,
        mime_type: String,
        target_device: Option<Uuid>,
        batch_id: Option<String>,
        is_directory: bool,
        item_count: u32,
    ) -> Result<[u8; 16]> {
        let mut mgr = self.shared.file_transfers.lock().await;
        let transfer = mgr.start_outbound_path(
            path,
            opened,
            file_name.clone(),
            mime_type,
            target_device,
            batch_id,
            is_directory,
            item_count,
        )?;
        let transfer_id = transfer.transfer_id;
        let meta = transfer.meta.clone();
        let size_bytes = meta.size_bytes;
        let _ = transfer;
        drop(mgr);

        self.announce_outbound_file_transfer(meta, file_name, size_bytes, target_device)
            .await?;
        Ok(transfer_id)
    }

    pub async fn accept_file_transfer(&self, transfer_id: [u8; 16]) -> Result<()> {
        let _ = {
            let mut mgr = self.shared.file_transfers.lock().await;
            mgr.queue_inbound(&transfer_id)
        };
        pump_transfer_queue(&self.shared).await;
        Ok(())
    }

    pub async fn pump_transfer_queue(&self) {
        let to_start = {
            let mgr = self.shared.file_transfers.lock().await;
            mgr.get_inbound_to_start(5) // Max 5 active inbound transfers
        };

        for (tid, peer_id) in to_start {
            let resume_from = {
                let session = self.shared.peer_manager.live_session_id(peer_id);
                let mut mgr = self.shared.file_transfers.lock().await;
                mgr.accept_inbound_or_resume(&tid, session).unwrap_or(0)
            };

            let accept_msg = AppMessage::FileTransferAccept {
                transfer_id: tid,
                accepted: true,
                resume_from_chunk: resume_from,
                reject_reason: None,
            };

            let peers = self.shared.peer_manager.all_trusted_senders();
            for (p_id, tx) in peers {
                if p_id == peer_id {
                    let _ = tx.try_send(accept_msg.clone());
                    break;
                }
            }
        }
    }

    pub(super) async fn announce_outbound_file_transfer(
        &self,
        meta: FileTransferMetadata,
        file_name: String,
        size_bytes: u64,
        target_device: Option<Uuid>,
    ) -> Result<()> {
        let transfer_id = meta.transfer_id;
        let announce = AppMessage::FileTransferAnnounce { meta };
        let peers = self.shared.peer_manager.all_connected_senders();
        let mut announced_to = 0usize;
        for (peer_id, tx) in peers {
            let should_send = match target_device {
                Some(t) => t == peer_id,
                None => true,
            };
            if !should_send {
                continue;
            }

            let msg = announce.clone();
            let send_result = match tx.try_send(msg.clone()) {
                Ok(()) => Ok(()),
                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => tx.send(msg).await,
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                    Err(tokio::sync::mpsc::error::SendError(msg))
                }
            };

            if send_result.is_ok() {
                announced_to += 1;
            } else {
                warn!(
                    "file transfer announce queue unavailable for peer {}",
                    peer_id
                );
            }
        }

        if announced_to == 0 {
            self.shared
                .file_transfers
                .lock()
                .await
                .cancel_outbound(&transfer_id);
            return Err(anyhow!("target peer queue unavailable"));
        }

        self.shared
            .activity
            .lock()
            .await
            .record_file_transfer_started(
                self.shared.config.device_id,
                self.shared.config.device_name.clone(),
                file_name,
                size_bytes,
                hex::encode(transfer_id),
                true,
            );
        Ok(())
    }

    /// Reject an incoming file transfer.
    pub async fn reject_file_transfer(&self, transfer_id: [u8; 16], reason: String) -> Result<()> {
        let from_device = {
            let mut mgr = self.shared.file_transfers.lock().await;
            let dev = mgr
                .all_inbound()
                .iter()
                .find(|t| t.transfer_id == transfer_id)
                .map(|t| t.from_device);
            mgr.reject_inbound(&transfer_id);
            dev
        };
        if let Some(from_device) = from_device {
            let reject_msg = AppMessage::FileTransferAccept {
                transfer_id,
                accepted: false,
                resume_from_chunk: 0,
                reject_reason: Some(reason),
            };
            let peers = self.shared.peer_manager.all_trusted_senders();
            for (peer_id, tx) in peers {
                if peer_id == from_device {
                    let _ = tx.try_send(reject_msg);
                    break;
                }
            }
        }
        Ok(())
    }

    /// Cancel an active file transfer (inbound or outbound).
    pub async fn cancel_file_transfer(&self, transfer_id: [u8; 16]) -> Result<()> {
        let cancel_msg = AppMessage::FileTransferCancel {
            transfer_id,
            reason: "user cancelled".into(),
        };
        // Cancel in manager.
        {
            let mut mgr = self.shared.file_transfers.lock().await;
            mgr.cancel_inbound(&transfer_id, "user cancelled");
            mgr.cancel_outbound(&transfer_id);
        }
        // Notify all peers.
        let peers = self.shared.peer_manager.all_trusted_senders();
        for (_, tx) in peers {
            let msg = cancel_msg.clone();
            tokio::spawn(async move {
                let _ = tx.send(msg).await;
            });
        }
        let _ = self
            .shared
            .event_tx
            .send(EngineEvent::FileTransferFailed {
                transfer_id,
                from_device: Uuid::nil(),
                reason: "User cancelled".to_string(),
            })
            .await;
        Ok(())
    }

    /// Pause an active file transfer.
    pub async fn pause_file_transfer(&self, transfer_id: [u8; 16]) -> Result<()> {
        let pause_msg = AppMessage::FileTransferPause { transfer_id };
        {
            let mut mgr = self.shared.file_transfers.lock().await;
            if let Some(t) = mgr.get_outbound_mut(&transfer_id) {
                t.paused = true;
            } else if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                t.paused = true;
            }
        }
        let peers = self.shared.peer_manager.all_trusted_senders();
        for (_, tx) in peers {
            let msg = pause_msg.clone();
            tokio::spawn(async move {
                let _ = tx.send(msg).await;
            });
        }
        let _ = self
            .shared
            .event_tx
            .send(EngineEvent::FileTransferPaused { transfer_id })
            .await;
        Ok(())
    }

    /// Resume a paused file transfer.
    pub async fn resume_file_transfer(&self, transfer_id: [u8; 16]) -> Result<()> {
        let resume_msg = AppMessage::FileTransferResume { transfer_id };
        let mut was_outbound = false;
        let mut target_device = None;
        let mut bg_send_run = 0;
        {
            let mut mgr = self.shared.file_transfers.lock().await;
            if let Some(t) = mgr.get_outbound_mut(&transfer_id) {
                let resume_chunk = t.last_acked_chunk.map(|c| c + 1).unwrap_or(0);
                t.resume_from(resume_chunk);
                t.paused = false;
                was_outbound = true;
                bg_send_run = t.start_send_run();
                target_device = t.target_device;
            } else if let Some(t) = mgr.get_inbound_mut(&transfer_id) {
                t.paused = false;
            }
        }
        let peers = self.shared.peer_manager.all_trusted_senders();
        for (peer_id, tx) in peers {
            let msg = resume_msg.clone();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                let _ = tx_clone.send(msg).await;
            });

            // If we are the sender, we need to restart the chunking loop!
            // `tx` is the `session_outbox_tx` for this peer.
            if was_outbound && target_device.map(|td| td == peer_id).unwrap_or(true) {
                let bg_outbox = self
                    .shared
                    .peer_manager
                    .file_sender(peer_id)
                    .unwrap_or(tx.clone());
                let bg_shared = self.shared.clone();
                let bg_event_tx = self.shared.event_tx.clone();
                let bg_transfer_id = transfer_id;
                let bg_peer_id = peer_id;
                let mut bg_last_prog_emit: std::collections::HashMap<[u8; 16], std::time::Instant> =
                    std::collections::HashMap::new();
                tokio::spawn(async move {
                    const BATCH_SIZE: usize = 4;
                    'outer: loop {
                        let (next_chunk, _last_acked, total_chunks, is_paused): (
                            u32,
                            u32,
                            u32,
                            bool,
                        ) = {
                            let mut mgr = bg_shared.file_transfers.lock().await;
                            if let Some(t) = mgr.get_outbound_mut(&bg_transfer_id) {
                                (
                                    t.next_chunk,
                                    t.last_acked_chunk.unwrap_or(0),
                                    t.total_chunks,
                                    t.paused,
                                )
                            } else {
                                break 'outer;
                            }
                        };
                        if is_paused || next_chunk >= total_chunks {
                            break 'outer;
                        }

                        let (batch, progs) = match read_outbound_chunks(
                            bg_shared.clone(),
                            bg_transfer_id,
                            BATCH_SIZE,
                            bg_send_run,
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
                                    now.checked_sub(std::time::Duration::from_secs(1)).unwrap()
                                });
                            if now.duration_since(last).as_millis() >= 100 || prog.percent == 100 {
                                bg_last_prog_emit.insert(bg_transfer_id, now);
                                let _ = bg_event_tx.try_send(EngineEvent::FileTransferProgress {
                                    transfer_id: bg_transfer_id,
                                    from_device: bg_peer_id,
                                    file_name: fname.clone(),
                                    percent: prog.percent,
                                    bytes_received: prog.bytes_received,
                                    total_bytes: prog.total_bytes,
                                    speed_bps: prog.speed_bps,
                                    eta_secs: prog.eta_secs,
                                    outbound: true,
                                });
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

        let _ = self
            .shared
            .event_tx
            .send(EngineEvent::FileTransferResumed { transfer_id })
            .await;
        Ok(())
    }
}

pub(crate) async fn pump_transfer_queue(shared: &EngineShared) {
    let to_start = {
        let mgr = shared.file_transfers.lock().await;
        mgr.get_inbound_to_start(5) // Max 5 active inbound transfers
    };

    for (tid, peer_id) in to_start {
        let resume_from = {
            let session = shared.peer_manager.live_session_id(peer_id);
            let mut mgr = shared.file_transfers.lock().await;
            mgr.accept_inbound_or_resume(&tid, session).unwrap_or(0)
        };

        let accept_msg = AppMessage::FileTransferAccept {
            transfer_id: tid,
            accepted: true,
            resume_from_chunk: resume_from,
            reject_reason: None,
        };

        let peers = shared.peer_manager.all_trusted_senders();
        for (p_id, tx) in peers {
            if p_id == peer_id {
                let _ = tx.try_send(accept_msg.clone());
                break;
            }
        }
    }
}
