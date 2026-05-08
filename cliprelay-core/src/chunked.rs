//! ClipRelay chunked file transfer — streaming, verified delivery.
//!
//! # Pipeline
//! ```
//! Sender                              Receiver
//! ──────                              ────────
//! FileTransferAnnounce ──────────────► (user accepts / auto-accept small files)
//!                      ◄────────────── FileTransferAccept { accepted: true }
//! ChunkStart ──────────────────────►
//! Chunk(0) ────────────────────────►
//! Chunk(1) ────────────────────────►
//! ...
//! ChunkEnd ────────────────────────►  reassemble → SHA-256 verify → save
//! ```
//!
//! Each chunk frame is individually encrypted by the session AEAD layer.
//! SHA-256 over the complete file is verified before the file is written.

use crate::protocol::ClipboardContent;
use anyhow::{Context, Result};
use bytes::Bytes;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

pub const CHUNK_THRESHOLD: usize = 128 * 1024; // 128 KB
pub const CHUNK_SIZE: usize = 64 * 1024; // 64 KB per chunk

pub type TransferId = [u8; 16];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum ChunkKind {
    Text,
    Image { mime: String },
    File { name: String },
}

// ── Sender ────────────────────────────────────────────────────────────────────

/// Split a `ClipboardContent` into ordered chunk messages.
/// Returns `None` for small payloads that can be sent inline.
pub fn maybe_chunk(content: &ClipboardContent) -> Option<Vec<ChunkMessage>> {
    let raw = match content {
        ClipboardContent::Text(s) => s.as_bytes().to_vec(),
        ClipboardContent::Image { data, .. } => data.clone(),
        ClipboardContent::File { data, .. } => data.clone(),
    };

    if raw.len() <= CHUNK_THRESHOLD {
        return None;
    }

    let kind = match content {
        ClipboardContent::Text(_) => ChunkKind::Text,
        ClipboardContent::Image { mime, .. } => ChunkKind::Image { mime: mime.clone() },
        ClipboardContent::File { name, .. } => ChunkKind::File { name: name.clone() },
    };

    let checksum = hex::encode(Sha256::digest(&raw));
    let mut id = [0u8; 16];
    id.copy_from_slice(Uuid::new_v4().as_bytes());

    let chunks: Vec<Bytes> = raw
        .chunks(CHUNK_SIZE)
        .map(|c| Bytes::copy_from_slice(c))
        .collect();
    let total_chunks = chunks.len() as u32;
    let total_bytes = raw.len() as u64;

    let mut msgs = Vec::with_capacity(chunks.len() + 2);
    msgs.push(ChunkMessage::Start {
        transfer_id: id,
        total_chunks,
        total_bytes,
        checksum,
        kind,
    });
    for (index, data) in chunks.into_iter().enumerate() {
        msgs.push(ChunkMessage::Chunk {
            transfer_id: id,
            index: index as u32,
            data: data.to_vec(),
        });
    }
    msgs.push(ChunkMessage::End { transfer_id: id });

    Some(msgs)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ChunkMessage {
    Start {
        transfer_id: TransferId,
        total_chunks: u32,
        total_bytes: u64,
        /// SHA-256 hex string of the complete payload (for integrity verification).
        checksum: String,
        kind: ChunkKind,
    },
    Chunk {
        transfer_id: TransferId,
        index: u32,
        data: Vec<u8>,
    },
    End {
        transfer_id: TransferId,
    },
}

// ── Receiver / Reassembler ────────────────────────────────────────────────────

#[derive(Debug)]
struct Transfer {
    kind: ChunkKind,
    total_chunks: u32,
    total_bytes: u64,
    expected_checksum: String,
    chunks: HashMap<u32, Vec<u8>>,
}

#[derive(Default)]
pub struct Reassembler {
    in_flight: HashMap<TransferId, Transfer>,
}

#[derive(Debug)]
pub enum ReassemblerOutput {
    InProgress {
        received: u32,
        total: u32,
        bytes_so_far: u64,
        percent: u8,
    },
    Complete(ClipboardContent),
    ChecksumMismatch {
        expected: String,
        got: String,
    },
}

impl Reassembler {
    pub fn feed(&mut self, msg: ChunkMessage) -> Result<Option<ReassemblerOutput>> {
        match msg {
            ChunkMessage::Start {
                transfer_id,
                total_chunks,
                total_bytes,
                checksum,
                kind,
            } => {
                self.in_flight.insert(
                    transfer_id,
                    Transfer {
                        kind,
                        total_chunks,
                        total_bytes,
                        expected_checksum: checksum,
                        chunks: HashMap::new(),
                    },
                );
                Ok(None)
            }

            ChunkMessage::Chunk {
                transfer_id,
                index,
                data,
            } => {
                let transfer = self
                    .in_flight
                    .get_mut(&transfer_id)
                    .context("chunk for unknown transfer")?;

                anyhow::ensure!(
                    index < transfer.total_chunks,
                    "chunk index {} out of range (total {})",
                    index,
                    transfer.total_chunks
                );
                anyhow::ensure!(
                    data.len() <= CHUNK_SIZE + 16,
                    "chunk {} too large ({})",
                    index,
                    data.len()
                );

                transfer.chunks.insert(index, data);
                let received = transfer.chunks.len() as u32;
                let bytes_so_far: u64 = transfer.chunks.values().map(|c| c.len() as u64).sum();
                let percent = ((received as f64 / transfer.total_chunks as f64) * 100.0) as u8;
                Ok(Some(ReassemblerOutput::InProgress {
                    received,
                    total: transfer.total_chunks,
                    bytes_so_far,
                    percent,
                }))
            }

            ChunkMessage::End { transfer_id } => {
                let transfer = self
                    .in_flight
                    .remove(&transfer_id)
                    .context("ChunkEnd for unknown transfer")?;

                anyhow::ensure!(
                    transfer.chunks.len() as u32 == transfer.total_chunks,
                    "missing chunks: got {} of {}",
                    transfer.chunks.len(),
                    transfer.total_chunks
                );

                let mut buf = Vec::with_capacity(transfer.total_bytes as usize);
                for i in 0..transfer.total_chunks {
                    let chunk = transfer
                        .chunks
                        .get(&i)
                        .context(format!("missing chunk {}", i))?;
                    buf.extend_from_slice(chunk);
                }

                anyhow::ensure!(
                    buf.len() as u64 == transfer.total_bytes,
                    "size mismatch: got {} expected {}",
                    buf.len(),
                    transfer.total_bytes
                );

                // ── SHA-256 integrity verification ────────────────────────────
                let actual_checksum = hex::encode(Sha256::digest(&buf));
                if actual_checksum != transfer.expected_checksum {
                    return Ok(Some(ReassemblerOutput::ChecksumMismatch {
                        expected: transfer.expected_checksum,
                        got: actual_checksum,
                    }));
                }

                let content = match transfer.kind {
                    ChunkKind::Text => {
                        let s = String::from_utf8(buf).context("chunk text not UTF-8")?;
                        ClipboardContent::Text(s)
                    }
                    ChunkKind::Image { mime } => ClipboardContent::Image { mime, data: buf },
                    ChunkKind::File { name } => ClipboardContent::File { name, data: buf },
                };

                Ok(Some(ReassemblerOutput::Complete(content)))
            }
        }
    }

    pub fn cancel(&mut self, transfer_id: &TransferId) {
        self.in_flight.remove(transfer_id);
    }

    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_content(size: usize) -> ClipboardContent {
        ClipboardContent::Text("A".repeat(size))
    }

    #[test]
    fn small_payload_is_not_chunked() {
        let c = make_content(1024);
        assert!(maybe_chunk(&c).is_none());
    }

    #[test]
    fn large_payload_roundtrip_with_checksum() {
        let original = make_content(CHUNK_THRESHOLD * 3 + 7777);
        let msgs = maybe_chunk(&original).expect("should chunk");
        assert!(msgs.len() > 3);

        let mut r = Reassembler::default();
        let mut result = None;
        for msg in msgs {
            match r.feed(msg).unwrap() {
                Some(ReassemblerOutput::Complete(c)) => result = Some(c),
                Some(ReassemblerOutput::ChecksumMismatch { .. }) => panic!("checksum mismatch"),
                _ => {}
            }
        }
        assert_eq!(result.unwrap(), original);
    }

    #[test]
    fn corrupted_transfer_detected() {
        let content = make_content(CHUNK_THRESHOLD * 2);
        let mut msgs = maybe_chunk(&content).unwrap();

        // Corrupt one data chunk
        for msg in &mut msgs {
            if let ChunkMessage::Chunk { data, .. } = msg {
                if !data.is_empty() {
                    data[0] ^= 0xFF;
                    break;
                }
            }
        }

        let mut r = Reassembler::default();
        let mut corrupted = false;
        for msg in msgs {
            match r.feed(msg).unwrap() {
                Some(ReassemblerOutput::ChecksumMismatch { .. }) => {
                    corrupted = true;
                }
                _ => {}
            }
        }
        assert!(corrupted, "should detect corruption via SHA-256");
    }
}
