import sys

with open("deskdrop-core/src/engine.rs", "r") as f:
    content = f.read()

# Replace first loop
loop1 = """                        let (batch, progresses): (Vec<AppMessage>, Vec<(crate::file_transfer::TransferProgress, String)>) = {
                            let mut mgr = bg_shared.file_transfers.lock().await;
                            let mut msgs = Vec::with_capacity(BATCH_SIZE);
                            let mut progs = Vec::with_capacity(BATCH_SIZE);
                            for _ in 0..BATCH_SIZE {
                                match mgr.get_outbound_mut(&bg_transfer_id) {
                                    Some(transfer) => match transfer.next_chunk_message() {
                                        Ok(Some(FileTransferMessage::Chunk {
                                            transfer_id,
                                            chunk_index,
                                            total_chunks,
                                            data,
                                        })) => {
                                            msgs.push(AppMessage::FileChunk {
                                                transfer_id,
                                                chunk_index,
                                                total_chunks,
                                                data,
                                            });
                                            progs.push((transfer.progress(), transfer.meta.file_name.clone()));
                                        }
                                        Ok(None) => break,
                                        Ok(_) => continue,
                                        Err(err) => {
                                            warn!(error = %err, "failed to read outbound file chunk on resume");
                                            mgr.cancel_outbound(&bg_transfer_id);
                                            break 'outer;
                                        }
                                    },
                                    None => break,
                                }
                            }
                            (msgs, progs)
                        };"""
replacement1 = """                        let (batch, progresses) = match read_outbound_chunks(bg_shared.clone(), bg_transfer_id, BATCH_SIZE).await {
                            Some(res) => res,
                            None => break 'outer,
                        };"""
content = content.replace(loop1, replacement1)

# Replace second loop
loop2 = """                                    let batch: Vec<AppMessage> = {
                                        let mut mgr = bg_shared.file_transfers.lock().await;
                                        let mut msgs = Vec::with_capacity(BATCH_SIZE);
                                        for _ in 0..BATCH_SIZE {
                                            match mgr.get_outbound_mut(&bg_transfer_id) {
                                                Some(transfer) => {
                                                    match transfer.next_chunk_message() {
                                                        Ok(Some(FileTransferMessage::Chunk {
                                                            transfer_id,
                                                            chunk_index,
                                                            total_chunks,
                                                            data,
                                                        })) => msgs.push(AppMessage::FileChunk {
                                                            transfer_id,
                                                            chunk_index,
                                                            total_chunks,
                                                            data,
                                                        }),
                                                        Ok(None) => break,
                                                        Ok(_) => continue,
                                                        Err(err) => {
                                                            warn!(error = %err, "failed to read outbound file chunk");
                                                            mgr.cancel_outbound(&bg_transfer_id);
                                                            break 'outer;
                                                        }
                                                    }
                                                }
                                                None => break,
                                            }
                                        }
                                        msgs
                                    };"""
replacement2 = """                                    let batch = match read_outbound_chunks(bg_shared.clone(), bg_transfer_id, BATCH_SIZE).await {
                                        Some((batch, _)) => batch,
                                        None => break 'outer,
                                    };"""
content = content.replace(loop2, replacement2)

# Replace third loop
loop3 = """                                    let (batch, progresses): (Vec<AppMessage>, Vec<(crate::file_transfer::TransferProgress, String)>) = {
                                        let mut mgr = bg_shared.file_transfers.lock().await;
                                        let mut msgs = Vec::with_capacity(BATCH_SIZE);
                                        let mut progs = Vec::with_capacity(BATCH_SIZE);
                                        for _ in 0..BATCH_SIZE {
                                            match mgr.get_outbound_mut(&bg_transfer_id) {
                                                Some(transfer) => {
                                                    match transfer.next_chunk_message() {
                                                        Ok(Some(FileTransferMessage::Chunk {
                                                            transfer_id,
                                                            chunk_index,
                                                            total_chunks,
                                                            data,
                                                        })) => {
                                                            msgs.push(AppMessage::FileChunk {
                                                                transfer_id,
                                                                chunk_index,
                                                                total_chunks,
                                                                data,
                                                            });
                                                            progs.push((transfer.progress(), transfer.meta.file_name.clone()));
                                                        }
                                                        Ok(None) => break,
                                                        Ok(_) => continue,
                                                        Err(err) => {
                                                            warn!(error = %err, "failed to read outbound file chunk on ack");
                                                            mgr.cancel_outbound(&bg_transfer_id);
                                                            break 'outer;
                                                        }
                                                    }
                                                }
                                                None => break,
                                            }
                                        }
                                        (msgs, progs)
                                    };"""
replacement3 = """                                    let (batch, progresses) = match read_outbound_chunks(bg_shared.clone(), bg_transfer_id, BATCH_SIZE).await {
                                        Some(res) => res,
                                        None => break 'outer,
                                    };"""
content = content.replace(loop3, replacement3)

helper = """

async fn read_outbound_chunks(
    shared: std::sync::Arc<crate::engine::SharedState>,
    transfer_id: [u8; 16],
    batch_size: usize,
) -> Option<(Vec<AppMessage>, Vec<(crate::file_transfer::TransferProgress, String)>)> {
    let mut msgs = Vec::with_capacity(batch_size);
    let mut progs = Vec::with_capacity(batch_size);
    for _ in 0..batch_size {
        let instr = {
            let mut mgr = shared.file_transfers.lock().await;
            let t = match mgr.get_outbound_mut(&transfer_id) {
                Some(t) => t,
                None => return if msgs.is_empty() { None } else { Some((msgs, progs)) },
            };
            match t.next_chunk_instruction() {
                Ok(Some(i)) => i,
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to get next chunk instruction");
                    mgr.cancel_outbound(&transfer_id);
                    return None;
                }
            }
        };
        
        let data = match instr {
            crate::file_transfer::ChunkInstruction::Memory { ref data, .. } => data.clone(),
            crate::file_transfer::ChunkInstruction::File { ref path, offset, len, .. } => {
                let path = path.clone();
                let res = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
                    use std::io::{Read, Seek};
                    let mut file = std::fs::File::open(&path)?;
                    file.seek(std::io::SeekFrom::Start(offset))?;
                    let mut buf = vec![0u8; len];
                    file.read_exact(&mut buf)?;
                    Ok(buf)
                }).await.unwrap();
                match res {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to read outbound file chunk");
                        let mut mgr = shared.file_transfers.lock().await;
                        mgr.cancel_outbound(&transfer_id);
                        return None;
                    }
                }
            }
        };
        
        let (msg, prog, fname) = {
            let mut mgr = shared.file_transfers.lock().await;
            let t = match mgr.get_outbound_mut(&transfer_id) {
                Some(t) => t,
                None => return if msgs.is_empty() { None } else { Some((msgs, progs)) },
            };
            let c_idx = match instr {
                crate::file_transfer::ChunkInstruction::Memory { chunk_index, .. } => chunk_index,
                crate::file_transfer::ChunkInstruction::File { chunk_index, .. } => chunk_index,
            };
            let msg = t.process_chunk_data(c_idx, data);
            (msg, t.progress(), t.meta.file_name.clone())
        };
        
        if let crate::protocol::FileTransferMessage::Chunk { transfer_id, chunk_index, total_chunks, data } = msg {
            msgs.push(AppMessage::FileChunk { transfer_id, chunk_index, total_chunks, data });
            progs.push((prog, fname));
        }
    }
    
    if msgs.is_empty() {
        None
    } else {
        Some((msgs, progs))
    }
}
"""

content = content + helper

with open("deskdrop-core/src/engine.rs", "w") as f:
    f.write(content)
