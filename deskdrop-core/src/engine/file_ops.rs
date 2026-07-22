use super::*;

pub(crate) async fn read_outbound_chunks(
    shared: crate::engine::EngineShared,
    transfer_id: [u8; 16],
    batch_size: usize,
) -> Option<(
    Vec<AppMessage>,
    Vec<(crate::file_transfer::TransferProgress, String)>,
)> {
    let mut instrs = Vec::with_capacity(batch_size);
    let io_ctx;

    {
        let mut mgr = shared.file_transfers.lock().await;
        let t = mgr.get_outbound_mut(&transfer_id)?;
        io_ctx = t.take_io_context();
        let effective_batch = t.adaptive_batch_size(batch_size);
        for _ in 0..effective_batch {
            match t.next_chunk_instruction() {
                Ok(Some(i)) => instrs.push(i),
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to get next chunk instruction");
                    mgr.cancel_outbound(&transfer_id);
                    return None;
                }
            }
        }
    }

    if instrs.is_empty() {
        if let Some((f, h)) = io_ctx {
            if let Some(t) = shared
                .file_transfers
                .lock()
                .await
                .get_outbound_mut(&transfer_id)
            {
                t.restore_io_context(f, h);
            }
        }
        return None;
    }

    type FileChunkResult = anyhow::Result<(
        Option<(Option<std::fs::File>, sha2::Sha256)>,
        Vec<(u32, bytes::Bytes, bool)>,
    )>;

    // Determine if we should try LZ4 based on file extension.
    // Already-compressed formats gain nothing from LZ4 and waste CPU.
    let try_compress = {
        let mgr = shared.file_transfers.lock().await;
        mgr.get_outbound(&transfer_id)
            .map(|t| should_try_compress(&t.meta.file_name))
            .unwrap_or(true)
    };

    let res = tokio::task::spawn_blocking(move || -> FileChunkResult {
        use sha2::Digest;
        use std::io::{Read, Seek};

        let mut chunk_data = Vec::with_capacity(instrs.len());
        let (mut f, mut hasher) = io_ctx.unwrap_or((None, sha2::Sha256::new())); // Memory chunks might not have io_ctx, but we'll return it anyway

        for instr in instrs {
            match instr {
                crate::file_transfer::ChunkInstruction::Memory { chunk_index, data } => {
                    hasher.update(&data);
                    let do_compress = if try_compress {
                        let sample_len = data.len().min(4096);
                        if sample_len > 0 {
                            let sample = &data[..sample_len];
                            let c_sample = lz4_flex::compress_prepend_size(sample);
                            c_sample.len() < sample_len * 95 / 100
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    if do_compress {
                        let compressed = lz4_flex::compress_prepend_size(&data);
                        if compressed.len() < data.len() {
                            chunk_data.push((chunk_index, bytes::Bytes::from(compressed), true));
                        } else {
                            chunk_data.push((chunk_index, data, false));
                        }
                    } else {
                        chunk_data.push((chunk_index, data, false));
                    }
                }
                crate::file_transfer::ChunkInstruction::File {
                    chunk_index,
                    path,
                    offset,
                    len,
                } => {
                    if f.is_none() {
                        f = Some(std::fs::File::open(&path)?);
                    }
                    if let Some(ref mut file) = f {
                        let current_pos = file.stream_position().unwrap_or(u64::MAX);
                        if current_pos != offset {
                            file.seek(std::io::SeekFrom::Start(offset))?;
                        }
                        let mut buf = vec![0u8; len];
                        let mut read_bytes = 0;
                        while read_bytes < len {
                            let n = file.read(&mut buf[read_bytes..])?;
                            if n == 0 {
                                break;
                            }
                            read_bytes += n;
                        }
                        if read_bytes < len {
                            tracing::warn!(
                                "Outbound chunk truncated: read {} instead of {} bytes",
                                read_bytes,
                                len
                            );
                            buf.truncate(read_bytes);
                        }
                        hasher.update(&buf);
                        let do_compress = if try_compress {
                            let sample_len = buf.len().min(4096);
                            if sample_len > 0 {
                                let sample = &buf[..sample_len];
                                let c_sample = lz4_flex::compress_prepend_size(sample);
                                c_sample.len() < sample_len * 95 / 100
                            } else {
                                false
                            }
                        } else {
                            false
                        };
                        if do_compress {
                            let compressed = lz4_flex::compress_prepend_size(&buf);
                            if compressed.len() < buf.len() {
                                chunk_data.push((
                                    chunk_index,
                                    bytes::Bytes::from(compressed),
                                    true,
                                ));
                            } else {
                                chunk_data.push((chunk_index, bytes::Bytes::from(buf), false));
                            }
                        } else {
                            chunk_data.push((chunk_index, bytes::Bytes::from(buf), false));
                        }
                    }
                }
            }
        }
        Ok((Some((f, hasher)), chunk_data))
    })
    .await
    .unwrap();

    let (io_ctx, chunk_data) = match res {
        Ok(res) => res,
        Err(e) => {
            tracing::warn!(error = %e, "failed to read outbound file chunks");
            let mut mgr = shared.file_transfers.lock().await;
            mgr.cancel_outbound(&transfer_id);
            return None;
        }
    };

    let mut msgs = Vec::with_capacity(chunk_data.len());
    let mut progs = Vec::with_capacity(chunk_data.len());

    {
        let mut mgr = shared.file_transfers.lock().await;
        let t = mgr.get_outbound_mut(&transfer_id)?;
        if let Some((f, h)) = io_ctx {
            t.restore_io_context(f, h);
        }
        let fname = t.meta.file_name.clone();
        for (c_idx, data, compressed) in chunk_data {
            let msg = t.process_chunk_data(c_idx, data, compressed);
            if let crate::file_transfer::FileTransferMessage::Chunk {
                transfer_id,
                chunk_index,
                total_chunks,
                data,
                compressed,
            } = msg
            {
                msgs.push(AppMessage::FileChunk {
                    transfer_id,
                    chunk_index,
                    total_chunks,
                    data,
                    compressed,
                });
                progs.push((t.progress(), fname.clone()));
            }
        }
    }

    if msgs.is_empty() {
        None
    } else {
        Some((msgs, progs))
    }
}

/// Returns false for file extensions that are already compressed.
/// LZ4 on these formats wastes CPU and always produces larger output.
fn should_try_compress(file_name: &str) -> bool {
    let ext = file_name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    !matches!(
        ext.as_str(),
        "jpg"
            | "jpeg"
            | "png"
            | "gif"
            | "webp"
            | "avif"
            | "heic"
            | "heif"
            | "mp4"
            | "mkv"
            | "mov"
            | "avi"
            | "webm"
            | "mp3"
            | "aac"
            | "ogg"
            | "opus"
            | "flac"
            | "m4a"
            | "wma"
            | "zip"
            | "gz"
            | "bz2"
            | "xz"
            | "zst"
            | "lz4"
            | "7z"
            | "rar"
            | "tar.gz"
            | "tgz"
            | "apk"
            | "ipa"
            | "dmg"
            | "iso"
            | "pdf"
    )
}
