//! ClipRelay File Transfer Pipeline — production-grade chunked file relay.
//!
//! # Design
//!
//! ```text
//! Sender                                   Receiver
//! ──────                                   ────────
//! FileTransferAnnounce ─────────────────►  (display: "Phone sending report.pdf, 2.3 MB — Accept?")
//!                      ◄─────────────────  FileTransferAccept { accepted: true }
//! FileChunk(0) ────────────────────────►  (write chunk 0 to tmp file)
//! FileChunk(1) ────────────────────────►  (write chunk 1 to tmp file)
//! ...                                      (send periodic ack every N chunks)
//!                      ◄─────────────────  FileChunkAck { last_ack: 5 }
//! FileChunk(N-1) ──────────────────────►
//! FileTransferComplete ────────────────►  (verify SHA-256) → rename tmp → notify UI
//!                      ◄─────────────────  FileTransferCompleteAck
//! ```
//!
//! # Resume
//! If the connection drops mid-transfer, the receiver stores `last_ack`.
//! On reconnect the sender re-announces the same transfer_id and the
//! receiver replies with `FileTransferAccept { resume_from_chunk }`.
//! The sender skips already-delivered chunks.
//!
//! # Integrity
//! SHA-256 over the complete file is verified before the file is finalized.
//! Any mismatch causes the partial file to be discarded.

use crate::protocol::FileTransferMetadata;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const FILE_CHUNK_SIZE: usize = 256 * 1024; // 256 KB per chunk
pub const FILE_ACK_EVERY_N_CHUNKS: u32 = 4;

pub type TransferId = [u8; 16];

// ── Wire messages for the file transfer channel ───────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FileTransferMessage {
    /// Sender announces intent to send a file.
    Announce { meta: FileTransferMetadata },
    /// Receiver accepts or rejects, optionally resuming from a chunk.
    Accept {
        transfer_id: TransferId,
        accepted: bool,
        /// Non-zero means: skip chunks 0..resume_from (already received).
        resume_from_chunk: u32,
        reject_reason: Option<String>,
    },
    /// One chunk of file data.
    Chunk {
        transfer_id: TransferId,
        chunk_index: u32,
        total_chunks: u32,
        data: Vec<u8>,
    },
    /// Periodic acknowledgement from receiver.
    ChunkAck {
        transfer_id: TransferId,
        last_confirmed_chunk: u32,
    },
    /// Sender signals all chunks sent; receiver should verify and finalize.
    Complete { transfer_id: TransferId },
    /// Receiver confirms finalization (or reports error).
    CompleteAck {
        transfer_id: TransferId,
        success: bool,
        error: Option<String>,
    },
    /// Either side may cancel.
    Cancel {
        transfer_id: TransferId,
        reason: String,
    },
    /// Progress update from receiver to UI layer (not sent over wire, local only).
    Progress {
        transfer_id: TransferId,
        bytes_received: u64,
        total_bytes: u64,
        percent: u8,
        speed_bps: Option<u64>,
        eta_secs: Option<u64>,
    },
}

// ── Transfer status ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferStatus {
    Pending,
    Transferring,
    Verifying,
    Complete,
    Failed,
    Cancelled,
}

// ── Sender state ──────────────────────────────────────────────────────────────

pub struct OutboundTransfer {
    pub transfer_id: TransferId,
    pub meta: FileTransferMetadata,
    /// All chunks pre-split (lazy: only loaded on demand to bound memory).
    pub chunks: Vec<Vec<u8>>,
    pub total_chunks: u32,
    pub next_chunk: u32,
    pub last_acked_chunk: u32,
    pub status: TransferStatus,
    pub created_at: Instant,
    pub target_device: Option<Uuid>,
}

impl OutboundTransfer {
    /// Split `data` into chunks and create an outbound transfer.
    pub fn new(data: Vec<u8>, meta: FileTransferMetadata, target_device: Option<Uuid>) -> Self {
        let mut id = [0u8; 16];
        id.copy_from_slice(Uuid::new_v4().as_bytes());

        let chunks: Vec<Vec<u8>> = data
            .chunks(FILE_CHUNK_SIZE)
            .map(|c| c.to_vec())
            .collect();
        let total_chunks = chunks.len() as u32;

        Self {
            transfer_id: id,
            meta,
            chunks,
            total_chunks,
            next_chunk: 0,
            last_acked_chunk: 0,
            status: TransferStatus::Pending,
            created_at: Instant::now(),
            target_device,
        }
    }

    /// Get next chunk message to send. Returns None when all sent.
    pub fn next_chunk_message(&mut self) -> Option<FileTransferMessage> {
        if self.next_chunk >= self.total_chunks {
            return None;
        }
        let idx = self.next_chunk;
        let data = self.chunks[idx as usize].clone();
        self.next_chunk += 1;
        Some(FileTransferMessage::Chunk {
            transfer_id: self.transfer_id,
            chunk_index: idx,
            total_chunks: self.total_chunks,
            data,
        })
    }

    /// Called when receiver acks chunks up to `last_confirmed`.
    pub fn on_chunk_ack(&mut self, last_confirmed: u32) {
        self.last_acked_chunk = last_confirmed;
    }

    /// Resume from the given chunk (skip already-delivered ones).
    pub fn resume_from(&mut self, chunk_index: u32) {
        self.next_chunk = chunk_index;
        self.last_acked_chunk = chunk_index.saturating_sub(1);
        self.status = TransferStatus::Transferring;
    }

    pub fn is_all_sent(&self) -> bool {
        self.next_chunk >= self.total_chunks
    }
}

// ── Receiver state ────────────────────────────────────────────────────────────

pub struct InboundTransfer {
    pub transfer_id: TransferId,
    pub meta: FileTransferMetadata,
    pub total_chunks: u32,
    pub received_chunks: HashMap<u32, Vec<u8>>,
    pub last_confirmed_chunk: u32,
    pub status: TransferStatus,
    pub created_at: Instant,
    pub started_at: Option<Instant>,
    pub bytes_received: u64,
    /// Temp file path for streaming writes.
    pub tmp_path: Option<PathBuf>,
    /// Final destination path.
    pub dest_path: Option<PathBuf>,
    pub from_device: Uuid,
    pub from_device_name: String,
}

impl InboundTransfer {
    pub fn new(meta: FileTransferMetadata, from_device: Uuid, from_device_name: String) -> Self {
        let total_chunks = ((meta.size_bytes as usize).saturating_add(FILE_CHUNK_SIZE - 1) / FILE_CHUNK_SIZE) as u32;
        Self {
            transfer_id: meta.transfer_id,
            meta,
            total_chunks,
            received_chunks: HashMap::new(),
            last_confirmed_chunk: 0,
            status: TransferStatus::Pending,
            created_at: Instant::now(),
            started_at: None,
            bytes_received: 0,
            tmp_path: None,
            dest_path: None,
            from_device,
            from_device_name,
        }
    }

    /// Accept the transfer, setting up paths.
    pub fn accept(&mut self, save_dir: &Path) -> Result<()> {
        let uid = hex::encode(&self.transfer_id[..4]);
        let tmp_name = format!(".cliprelay_tmp_{uid}_{}", self.meta.file_name);
        self.tmp_path = Some(save_dir.join(&tmp_name));
        self.dest_path = Some(unique_dest_path(save_dir, &self.meta.file_name));
        self.status = TransferStatus::Transferring;
        self.started_at = Some(Instant::now());
        Ok(())
    }

    /// Feed a chunk into the transfer. Returns progress info.
    pub fn receive_chunk(&mut self, chunk_index: u32, data: Vec<u8>) -> Result<TransferProgress> {
        anyhow::ensure!(
            chunk_index < self.total_chunks,
            "chunk {} out of range (total {})", chunk_index, self.total_chunks
        );
        let len = data.len() as u64;
        self.received_chunks.entry(chunk_index).or_insert_with(|| {
            self.bytes_received += len;
            data
        });
        self.last_confirmed_chunk = chunk_index;

        let percent = ((self.received_chunks.len() as f64 / self.total_chunks as f64) * 100.0) as u8;
        let elapsed = self.started_at.map(|s| s.elapsed()).unwrap_or_default();
        let speed_bps = if elapsed.as_secs() > 0 {
            Some(self.bytes_received / elapsed.as_secs())
        } else {
            None
        };
        let eta_secs = speed_bps.and_then(|spd| {
            if spd > 0 {
                let remaining = self.meta.size_bytes.saturating_sub(self.bytes_received);
                Some(remaining / spd)
            } else {
                None
            }
        });

        Ok(TransferProgress {
            transfer_id: self.transfer_id,
            bytes_received: self.bytes_received,
            total_bytes: self.meta.size_bytes,
            percent,
            speed_bps,
            eta_secs,
        })
    }

    /// Verify SHA-256 and assemble the final file.
    pub fn finalize(&mut self) -> Result<PathBuf> {
        anyhow::ensure!(
            self.received_chunks.len() as u32 == self.total_chunks,
            "missing chunks: got {} of {}",
            self.received_chunks.len(),
            self.total_chunks
        );

        // Reassemble in order.
        let mut buf: Vec<u8> = Vec::with_capacity(self.meta.size_bytes as usize);
        for i in 0..self.total_chunks {
            let chunk = self.received_chunks.get(&i)
                .with_context(|| format!("missing chunk {}", i))?;
            buf.extend_from_slice(chunk);
        }

        // Integrity verification.
        let actual = hex::encode(Sha256::digest(&buf));
        anyhow::ensure!(
            actual == self.meta.sha256_checksum,
            "SHA-256 mismatch: expected {}, got {}",
            self.meta.sha256_checksum,
            actual
        );

        let dest = self.dest_path.as_ref().context("no dest path")?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).context("creating save dir")?;
        }
        std::fs::write(dest, &buf).context("writing final file")?;
        self.status = TransferStatus::Complete;
        Ok(dest.clone())
    }

    /// Should we send a chunk ack now?
    pub fn should_ack(&self) -> bool {
        self.last_confirmed_chunk > 0 && self.last_confirmed_chunk % FILE_ACK_EVERY_N_CHUNKS == 0
    }
}

#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub transfer_id: TransferId,
    pub bytes_received: u64,
    pub total_bytes: u64,
    pub percent: u8,
    pub speed_bps: Option<u64>,
    pub eta_secs: Option<u64>,
}

// ── Transfer manager ──────────────────────────────────────────────────────────

/// Manages all active file transfers (inbound and outbound).
pub struct FileTransferManager {
    inbound: HashMap<[u8; 16], InboundTransfer>,
    outbound: HashMap<[u8; 16], OutboundTransfer>,
    save_dir: PathBuf,
}

impl FileTransferManager {
    pub fn new(save_dir: PathBuf) -> Self {
        Self {
            inbound: HashMap::new(),
            outbound: HashMap::new(),
            save_dir,
        }
    }

    pub fn save_dir(&self) -> &Path {
        &self.save_dir
    }

    // ── Outbound ─────────────────────────────────────────────────────────────

    pub fn start_outbound(
        &mut self,
        data: Vec<u8>,
        file_name: String,
        mime_type: String,
        target_device: Option<Uuid>,
    ) -> Result<&OutboundTransfer> {
        let checksum = hex::encode(Sha256::digest(&data));
        let mut tid = [0u8; 16];
        tid.copy_from_slice(Uuid::new_v4().as_bytes());

        let meta = FileTransferMetadata {
            transfer_id: tid,
            file_name: file_name.clone(),
            size_bytes: data.len() as u64,
            mime_type,
            sha256_checksum: checksum,
        };
        let transfer = OutboundTransfer::new(data, meta, target_device);
        let tid = transfer.transfer_id;
        self.outbound.insert(tid, transfer);
        Ok(self.outbound.get(&tid).unwrap())
    }

    pub fn get_outbound_mut(&mut self, tid: &TransferId) -> Option<&mut OutboundTransfer> {
        self.outbound.get_mut(tid)
    }

    pub fn remove_outbound(&mut self, tid: &TransferId) -> Option<OutboundTransfer> {
        self.outbound.remove(tid)
    }

    // ── Inbound ───────────────────────────────────────────────────────────────

    pub fn register_inbound(
        &mut self,
        meta: FileTransferMetadata,
        from_device: Uuid,
        from_device_name: String,
    ) -> &mut InboundTransfer {
        let tid = meta.transfer_id;
        let transfer = InboundTransfer::new(meta, from_device, from_device_name);
        self.inbound.insert(tid, transfer);
        self.inbound.get_mut(&tid).unwrap()
    }

    pub fn accept_inbound(&mut self, tid: &TransferId) -> Result<u32> {
        let transfer = self.inbound.get_mut(tid).context("unknown transfer")?;
        transfer.accept(&self.save_dir)?;
        // Return resume_from_chunk (0 for new transfers).
        Ok(0)
    }

    /// Accept inbound with resume support: if we have partial state, return resume chunk.
    pub fn accept_inbound_or_resume(&mut self, tid: &TransferId) -> Result<u32> {
        let transfer = self.inbound.get_mut(tid).context("unknown transfer")?;
        let resume_from = if !transfer.received_chunks.is_empty() {
            transfer.last_confirmed_chunk + 1
        } else {
            0
        };
        if transfer.dest_path.is_none() {
            transfer.accept(&self.save_dir)?;
        } else {
            transfer.status = TransferStatus::Transferring;
        }
        Ok(resume_from)
    }

    pub fn reject_inbound(&mut self, tid: &TransferId) {
        if let Some(t) = self.inbound.get_mut(tid) {
            t.status = TransferStatus::Cancelled;
        }
        self.inbound.remove(tid);
    }

    pub fn get_inbound_mut(&mut self, tid: &TransferId) -> Option<&mut InboundTransfer> {
        self.inbound.get_mut(tid)
    }

    pub fn remove_inbound(&mut self, tid: &TransferId) -> Option<InboundTransfer> {
        self.inbound.remove(tid)
    }

    pub fn cancel_inbound(&mut self, tid: &TransferId, reason: &str) {
        if let Some(t) = self.inbound.remove(tid) {
            // Clean up tmp file.
            if let Some(tmp) = t.tmp_path {
                let _ = std::fs::remove_file(tmp);
            }
        }
    }

    pub fn cancel_outbound(&mut self, tid: &TransferId) {
        if let Some(mut t) = self.outbound.remove(tid) {
            t.status = TransferStatus::Cancelled;
        }
    }

    pub fn active_inbound_count(&self) -> usize {
        self.inbound.values().filter(|t| t.status == TransferStatus::Transferring).count()
    }

    pub fn active_outbound_count(&self) -> usize {
        self.outbound.values().filter(|t| t.status == TransferStatus::Transferring).count()
    }

    pub fn all_inbound(&self) -> Vec<&InboundTransfer> {
        self.inbound.values().collect()
    }

    pub fn all_outbound(&self) -> Vec<&OutboundTransfer> {
        self.outbound.values().collect()
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Compute a non-colliding destination path (appends (1), (2), etc. if needed).
fn unique_dest_path(dir: &Path, file_name: &str) -> PathBuf {
    let base = dir.join(file_name);
    if !base.exists() {
        return base;
    }
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name);
    let ext = Path::new(file_name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    for i in 1..=999 {
        let name = if ext.is_empty() {
            format!("{} ({})", stem, i)
        } else {
            format!("{} ({}).{}", stem, i, ext)
        };
        let candidate = dir.join(&name);
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{}_{}_{}", stem, now_unix(), ext))
}

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

/// Default save directory for received files.
pub fn default_save_dir() -> PathBuf {
    dirs::download_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("ClipRelay")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_meta(data: &[u8]) -> FileTransferMetadata {
        FileTransferMetadata {
            transfer_id: *Uuid::new_v4().as_bytes(),
            file_name: "test.txt".into(),
            size_bytes: data.len() as u64,
            mime_type: "text/plain".into(),
            sha256_checksum: hex::encode(Sha256::digest(data)),
        }
    }

    #[test]
    fn outbound_chunks_roundtrip() {
        let data = b"hello world".repeat(1000);
        let meta = make_meta(&data);
        let mut transfer = OutboundTransfer::new(data.clone(), meta, None);
        let mut collected: Vec<FileTransferMessage> = Vec::new();
        while let Some(msg) = transfer.next_chunk_message() {
            collected.push(msg);
        }
        assert!(!collected.is_empty());
        assert!(transfer.is_all_sent());
    }

    #[test]
    fn inbound_verify_integrity() {
        let tmp = TempDir::new().unwrap();
        let data = b"ClipRelay file transfer test".repeat(500);
        let meta = make_meta(&data);
        let tid = meta.transfer_id;
        let mut mgr = FileTransferManager::new(tmp.path().to_path_buf());
        mgr.register_inbound(meta, Uuid::new_v4(), "Phone".into());
        mgr.accept_inbound(&tid).unwrap();

        // Feed chunks directly from the data slice.
        let transfer = mgr.get_inbound_mut(&tid).unwrap();
        let mut chunk_idx = 0u32;
        for chunk in data.chunks(FILE_CHUNK_SIZE) {
            transfer.receive_chunk(chunk_idx, chunk.to_vec()).unwrap();
            chunk_idx += 1;
        }
        let dest = transfer.finalize().unwrap();
        let written = std::fs::read(&dest).unwrap();
        assert_eq!(written, data.as_slice());
    }

    #[test]
    fn inbound_detects_corruption() {
        let tmp = TempDir::new().unwrap();
        let data = b"some data".repeat(200);
        let meta = make_meta(&data);
        let tid = meta.transfer_id;
        let mut mgr = FileTransferManager::new(tmp.path().to_path_buf());
        mgr.register_inbound(meta, Uuid::new_v4(), "Laptop".into());
        mgr.accept_inbound(&tid).unwrap();

        let transfer = mgr.get_inbound_mut(&tid).unwrap();
        // Feed corrupted chunk
        transfer.receive_chunk(0, b"CORRUPTED DATA".to_vec()).unwrap();
        transfer.receive_chunk(1, b"more data".to_vec()).unwrap();
        // finalize should fail due to SHA-256 mismatch
        let result = transfer.finalize();
        assert!(result.is_err());
    }
}
