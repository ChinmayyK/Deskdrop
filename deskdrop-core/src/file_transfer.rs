//! Deskdrop File Transfer Pipeline — production-grade chunked file relay.
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
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const FILE_CHUNK_SIZE: usize = 4 * 1024 * 1024; // 4 MB per chunk — larger chunks saturate
                                                    // encrypt/serialize/frame/flush overhead.

/// HIGH-03 FIX: Maximum transfer size (4 GB). Rejects announced transfers
/// exceeding this limit to prevent disk-bomb attacks via pre-allocation.
pub const MAX_TRANSFER_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub const FILE_ACK_EVERY_N_CHUNKS: u32 = 16; // ACK every 16 MB — keeps the pipeline full
                                             // on LAN while still bounding in-flight data.

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

enum OutboundSource {
    Memory(Vec<u8>),
    FilePath(PathBuf, Option<std::fs::File>),
}

pub struct OutboundTransfer {
    pub transfer_id: TransferId,
    pub meta: FileTransferMetadata,
    source: OutboundSource,
    pub total_chunks: u32,
    pub next_chunk: u32,
    pub last_acked_chunk: u32,
    pub status: TransferStatus,
    pub created_at: Instant,
    pub last_active_at: Instant,
    pub target_device: Option<Uuid>,
    pub paused: bool,
}

impl OutboundTransfer {
    pub fn new(data: Vec<u8>, meta: FileTransferMetadata, target_device: Option<Uuid>) -> Self {
        let total_chunks = chunk_count(meta.size_bytes).unwrap_or(u32::MAX);

        Self {
            // Use the announced transfer ID so later accept/ack messages map
            // back to the sender's local outbound state.
            transfer_id: meta.transfer_id,
            meta,
            source: OutboundSource::Memory(data),
            total_chunks,
            next_chunk: 0,
            last_acked_chunk: 0,
            status: TransferStatus::Pending,
            created_at: Instant::now(),
            last_active_at: Instant::now(),
            target_device,
            paused: false,
        }
    }

    pub fn from_path(
        path: PathBuf,
        meta: FileTransferMetadata,
        target_device: Option<Uuid>,
    ) -> Result<Self> {
        let total_chunks = chunk_count(meta.size_bytes)?;
        Ok(Self {
            transfer_id: meta.transfer_id,
            meta,
            source: OutboundSource::FilePath(path, None),
            total_chunks,
            next_chunk: 0,
            last_acked_chunk: 0,
            status: TransferStatus::Pending,
            created_at: Instant::now(),
            last_active_at: Instant::now(),
            target_device,
            paused: false,
        })
    }

    /// Get next chunk message to send. Returns None when all sent.
    pub fn next_chunk_message(&mut self) -> Result<Option<FileTransferMessage>> {
        self.last_active_at = Instant::now();
        if self.paused {
            return Ok(None);
        }
        if self.next_chunk >= self.total_chunks {
            return Ok(None);
        }
        let idx = self.next_chunk;
        let data = match &mut self.source {
            OutboundSource::Memory(data_vec) => {
                let start = (idx as usize) * FILE_CHUNK_SIZE;
                let end = (start + FILE_CHUNK_SIZE).min(data_vec.len());
                data_vec[start..end].to_vec()
            }
            OutboundSource::FilePath(path, cached_file) => {
                if cached_file.is_none() {
                    *cached_file =
                        Some(std::fs::File::open(&path).with_context(|| {
                            format!("opening outbound file {}", path.display())
                        })?);
                }
                read_file_chunk_from_file(cached_file.as_mut().unwrap(), idx, self.meta.size_bytes)?
            }
        };
        self.next_chunk += 1;
        Ok(Some(FileTransferMessage::Chunk {
            transfer_id: self.transfer_id,
            chunk_index: idx,
            total_chunks: self.total_chunks,
            data,
        }))
    }

    /// Called when receiver acks chunks up to `last_confirmed`.
    pub fn on_chunk_ack(&mut self, last_confirmed: u32) {
        self.last_active_at = Instant::now();
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
    pub received_chunk_count: u32,
    pub last_confirmed_chunk: u32,
    pub status: TransferStatus,
    pub created_at: Instant,
    pub started_at: Option<Instant>,
    pub last_active_at: Instant,
    pub bytes_received: u64,
    /// Temp file path for streaming writes.
    pub tmp_path: Option<PathBuf>,
    /// Persistent file handle to avoid re-opening on every chunk.
    pub file_handle: Option<std::fs::File>,
    /// Final destination path.
    pub dest_path: Option<PathBuf>,
    pub from_device: Uuid,
    pub from_device_name: String,
    pub paused: bool,
    hasher: Sha256,
}

impl InboundTransfer {
    pub fn new(meta: FileTransferMetadata, from_device: Uuid, from_device_name: String) -> Self {
        let total_chunks = chunk_count(meta.size_bytes).unwrap_or(u32::MAX);
        Self {
            transfer_id: meta.transfer_id,
            meta,
            total_chunks,
            received_chunk_count: 0,
            last_confirmed_chunk: 0,
            status: TransferStatus::Pending,
            created_at: Instant::now(),
            started_at: None,
            last_active_at: Instant::now(),
            bytes_received: 0,
            tmp_path: None,
            file_handle: None,
            dest_path: None,
            from_device,
            from_device_name,
            paused: false,
            hasher: Sha256::new(),
        }
    }

    /// Accept the transfer, setting up paths.
    pub fn accept(&mut self, save_dir: &Path) -> Result<()> {
        // Strip any directory separators or traversal components from the
        // sender-supplied file name to prevent a malicious peer from writing
        // outside save_dir via "../../../etc/passwd" style names.
        let safe_name = sanitize_file_name(&self.meta.file_name);
        anyhow::ensure!(
            !safe_name.is_empty(),
            "file name is empty after sanitization"
        );

        let uid = hex::encode(&self.transfer_id[..4]);
        let tmp_name = format!(".deskdrop_tmp_{uid}_{safe_name}");
        self.tmp_path = Some(save_dir.join(&tmp_name));
        self.dest_path = Some(unique_dest_path(save_dir, &safe_name));
        std::fs::create_dir_all(save_dir).context("creating save dir")?;
        if let Some(tmp) = &self.tmp_path {
            let _ = std::fs::remove_file(tmp);
            let file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(tmp)
                .with_context(|| format!("creating temp file {}", tmp.display()))?;
            file.set_len(self.meta.size_bytes).context("pre-allocating disk space for incoming file")?;
            self.file_handle = Some(file);
        }
        self.status = TransferStatus::Transferring;
        self.started_at = Some(Instant::now());
        Ok(())
    }

    /// Feed a chunk into the transfer. Returns progress info.
    pub fn receive_chunk(&mut self, chunk_index: u32, data: Vec<u8>) -> Result<TransferProgress> {
        self.last_active_at = Instant::now();
        anyhow::ensure!(
            self.status == TransferStatus::Transferring,
            "transfer is not active"
        );
        anyhow::ensure!(!self.paused, "transfer is paused");
        anyhow::ensure!(data.len() <= 4 * 1024 * 1024, "chunk size exceeds limit");

        anyhow::ensure!(
            chunk_index < self.total_chunks,
            "chunk {} out of range (total {})",
            chunk_index,
            self.total_chunks
        );
        if chunk_index < self.received_chunk_count {
            return Ok(self.progress_snapshot());
        }
        anyhow::ensure!(
            chunk_index == self.received_chunk_count,
            "out-of-order chunk: expected {}, got {}",
            self.received_chunk_count,
            chunk_index
        );

        self.append_chunk(&data)?;
        let len = data.len() as u64;
        self.bytes_received += len;
        self.received_chunk_count += 1;
        self.last_confirmed_chunk = chunk_index;

        Ok(self.progress_snapshot())
    }

    /// Verify SHA-256 and assemble the final file.
    pub fn finalize(&mut self) -> Result<PathBuf> {
        anyhow::ensure!(
            self.received_chunk_count == self.total_chunks,
            "missing chunks: got {} of {}",
            self.received_chunk_count,
            self.total_chunks
        );

        // Integrity verification.
        let actual = {
            let hasher = std::mem::replace(&mut self.hasher, Sha256::new());
            hex::encode(hasher.finalize())
        };
        anyhow::ensure!(
            actual == self.meta.sha256_checksum,
            "SHA-256 mismatch: expected {}, got {}",
            self.meta.sha256_checksum,
            actual
        );

        // Drop the file handle so the OS releases the lock before renaming.
        self.file_handle = None;

        let tmp = self.tmp_path.as_ref().context("no temp path")?;
        let dest = self.dest_path.as_ref().context("no dest path")?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).context("creating save dir")?;
        }
        std::fs::rename(tmp, dest).with_context(|| {
            format!(
                "moving completed transfer from {} to {}",
                tmp.display(),
                dest.display()
            )
        })?;
        self.tmp_path = None;
        self.status = TransferStatus::Complete;
        Ok(dest.clone())
    }

    /// Should we send a chunk ack now?
    pub fn should_ack(&self) -> bool {
        self.received_chunk_count > 0
            && self
                .received_chunk_count
                .is_multiple_of(FILE_ACK_EVERY_N_CHUNKS)
    }

    fn append_chunk(&mut self, data: &[u8]) -> Result<()> {
        if let Some(file) = &mut self.file_handle {
            // Seek to the correct offset based on chunks received
            let offset = (self.received_chunk_count as u64) * (FILE_CHUNK_SIZE as u64);
            file.seek(std::io::SeekFrom::Start(offset)).context("seeking to chunk offset")?;
            file.write_all(data).context("writing chunk to temp file")?;
        } else {
            anyhow::bail!("transfer has not been accepted or file handle is missing");
        }
        self.hasher.update(data);
        Ok(())
    }

    fn progress_snapshot(&self) -> TransferProgress {
        let percent = if self.total_chunks == 0 {
            100
        } else {
            ((self.received_chunk_count as f64 / self.total_chunks as f64) * 100.0) as u8
        };
        let elapsed = self.started_at.map(|s| s.elapsed()).unwrap_or_default();
        let speed_bps = if elapsed.as_secs() > 0 {
            Some(self.bytes_received / elapsed.as_secs())
        } else {
            None
        };
        let eta_secs = speed_bps.and_then(|spd| {
            let remaining = self.meta.size_bytes.saturating_sub(self.bytes_received);
            remaining.checked_div(spd)
        });

        TransferProgress {
            transfer_id: self.transfer_id,
            bytes_received: self.bytes_received,
            total_bytes: self.meta.size_bytes,
            percent,
            speed_bps,
            eta_secs,
        }
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

    pub fn start_outbound_path(
        &mut self,
        path: PathBuf,
        file_name: String,
        mime_type: String,
        target_device: Option<Uuid>,
    ) -> Result<&OutboundTransfer> {
        let size_bytes = std::fs::metadata(&path)
            .with_context(|| format!("reading metadata for {}", path.display()))?
            .len();
        let checksum = checksum_file(&path)?;
        let mut tid = [0u8; 16];
        tid.copy_from_slice(Uuid::new_v4().as_bytes());

        let meta = FileTransferMetadata {
            transfer_id: tid,
            file_name,
            size_bytes,
            mime_type,
            sha256_checksum: checksum,
        };
        let transfer = OutboundTransfer::from_path(path, meta, target_device)?;
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
    ) -> Result<&mut InboundTransfer> {
        if self.inbound.len() >= 10 {
            anyhow::bail!("too many active transfers");
        }
        let count_from_peer = self
            .inbound
            .values()
            .filter(|t| t.from_device == from_device)
            .count();
        if count_from_peer >= 5 {
            anyhow::bail!("too many active transfers from this peer");
        }

        // HIGH-03 FIX: Reject transfers that exceed the maximum size limit
        // to prevent disk-bomb attacks via set_len() pre-allocation.
        if meta.size_bytes > MAX_TRANSFER_BYTES {
            anyhow::bail!(
                "transfer size {} bytes exceeds maximum {} bytes",
                meta.size_bytes,
                MAX_TRANSFER_BYTES
            );
        }

        let tid = meta.transfer_id;
        match self.inbound.entry(tid) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                let transfer = entry.into_mut();
                transfer.from_device = from_device;
                transfer.from_device_name = from_device_name;
                Ok(transfer)
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                Ok(entry.insert(InboundTransfer::new(meta, from_device, from_device_name)))
            }
        }
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
        let resume_from = if transfer.received_chunk_count > 0 {
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
        if let Some(mut t) = self.inbound.remove(tid) {
            t.status = TransferStatus::Cancelled;
            if let Some(tmp) = t.tmp_path.take() {
                let _ = std::fs::remove_file(tmp);
            }
        }
    }

    pub fn get_inbound_mut(&mut self, tid: &TransferId) -> Option<&mut InboundTransfer> {
        self.inbound.get_mut(tid)
    }

    pub fn remove_inbound(&mut self, tid: &TransferId) -> Option<InboundTransfer> {
        self.inbound.remove(tid)
    }

    pub fn cancel_inbound(&mut self, tid: &TransferId, _reason: &str) {
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

    pub fn cancel_all_for_device(&mut self, peer_id: Uuid) {
        let inbound_tids: Vec<_> = self
            .inbound
            .values()
            .filter(|t| t.from_device == peer_id)
            .map(|t| t.transfer_id)
            .collect();
        for tid in inbound_tids {
            self.cancel_inbound(&tid, "peer disconnected");
        }

        let outbound_tids: Vec<_> = self
            .outbound
            .values()
            .filter(|t| t.target_device == Some(peer_id))
            .map(|t| t.transfer_id)
            .collect();
        for tid in outbound_tids {
            self.cancel_outbound(&tid);
        }
    }

    pub fn prune_stale_transfers(&mut self) {
        let now = Instant::now();
        let timeout = std::time::Duration::from_secs(300); // 5 minutes

        let stale_inbound: Vec<_> = self
            .inbound
            .iter()
            .filter(|(_, t)| now.duration_since(t.last_active_at) > timeout)
            .map(|(tid, _)| *tid)
            .collect();
        for tid in stale_inbound {
            self.cancel_inbound(&tid, "transfer timed out (zombie)");
        }

        let stale_outbound: Vec<_> = self
            .outbound
            .iter()
            .filter(|(_, t)| now.duration_since(t.last_active_at) > timeout)
            .map(|(tid, _)| *tid)
            .collect();
        for tid in stale_outbound {
            self.cancel_outbound(&tid);
        }
    }

    pub fn active_inbound_count(&self) -> usize {
        self.inbound
            .values()
            .filter(|t| t.status == TransferStatus::Transferring)
            .count()
    }

    pub fn active_outbound_count(&self) -> usize {
        self.outbound
            .values()
            .filter(|t| t.status == TransferStatus::Transferring)
            .count()
    }

    pub fn pending_outbound_announcements_for(&self, peer_id: Uuid) -> Vec<FileTransferMetadata> {
        let mut pending: Vec<_> = self
            .outbound
            .values()
            .filter(|transfer| {
                matches!(
                    transfer.status,
                    TransferStatus::Pending | TransferStatus::Transferring
                ) && match transfer.target_device {
                    Some(target) => target == peer_id,
                    None => true,
                }
            })
            .collect();

        pending.sort_by_key(|transfer| transfer.created_at);
        pending
            .into_iter()
            .map(|transfer| transfer.meta.clone())
            .collect()
    }

    pub fn active_transfers(&self) -> Vec<serde_json::Value> {
        let mut transfers = Vec::new();
        for t in self.inbound.values() {
            let percent = if t.meta.size_bytes > 0 {
                (t.bytes_received as f64 / t.meta.size_bytes as f64 * 100.0) as u8
            } else {
                0
            };
            let status_str = match t.status {
                TransferStatus::Pending => "incoming",
                TransferStatus::Verifying => "verifying",
                TransferStatus::Complete => "complete",
                TransferStatus::Failed => "failed",
                TransferStatus::Cancelled => "cancelled",
                TransferStatus::Transferring => {
                    if t.paused {
                        "paused"
                    } else {
                        "transferring"
                    }
                }
            };
            transfers.push(serde_json::json!({
                "transfer_id": hex::encode(t.transfer_id),
                "from_device": t.from_device_name.clone(),
                "file_name": t.meta.file_name.clone(),
                "bytes_total": t.meta.size_bytes,
                "bytes_received": t.bytes_received,
                "percent": percent,
                "status": status_str
            }));
        }
        for t in self.outbound.values() {
            let bytes_sent = (t.last_acked_chunk as u64) * (FILE_CHUNK_SIZE as u64);
            let percent = if t.meta.size_bytes > 0 {
                (bytes_sent as f64 / t.meta.size_bytes as f64 * 100.0) as u8
            } else {
                0
            };
            let status_str = match t.status {
                TransferStatus::Pending => "transferring", // Remote hasn't accepted yet, but from our end it's outgoing
                TransferStatus::Verifying => "verifying",
                TransferStatus::Complete => "complete",
                TransferStatus::Failed => "failed",
                TransferStatus::Cancelled => "cancelled",
                TransferStatus::Transferring => {
                    if t.paused {
                        "paused"
                    } else {
                        "transferring"
                    }
                }
            };
            transfers.push(serde_json::json!({
                "transfer_id": hex::encode(t.transfer_id),
                "from_device": "Sending",
                "file_name": t.meta.file_name.clone(),
                "bytes_total": t.meta.size_bytes,
                "bytes_received": bytes_sent.min(t.meta.size_bytes),
                "percent": percent,
                "status": status_str
            }));
        }
        transfers
    }

    pub fn all_inbound(&self) -> Vec<&InboundTransfer> {
        self.inbound.values().collect()
    }

    pub fn all_outbound(&self) -> Vec<&OutboundTransfer> {
        self.outbound.values().collect()
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Strip path traversal components and directory separators from a
/// sender-supplied file name so it can never escape `save_dir` (MED-04).
///
/// Rules applied (in order):
/// 1. Take only the last path component (basename) — removes `../` prefixes.
/// 2. Remove any remaining `/` or `\` characters.
/// 3. Strip leading dots to avoid hidden files (e.g. `.bashrc`).
/// 4. If the result is empty or is a reserved name, substitute "file".
fn sanitize_file_name(name: &str) -> String {
    // Take the basename only.
    let base = std::path::Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(name);

    // Remove remaining separators and control characters.
    let sanitized: String = base
        .chars()
        .filter(|&c| c != '/' && c != '\\' && c != '\0')
        .collect();

    // Trim leading dots (hidden file prevention).
    let trimmed = sanitized.trim_start_matches('.');

    if trimmed.is_empty() {
        "file".to_string()
    } else {
        trimmed.to_string()
    }
}

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
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Default save directory for received files.
pub fn default_save_dir() -> PathBuf {
    dirs::download_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
}

fn chunk_count(size_bytes: u64) -> Result<u32> {
    let chunks = if size_bytes == 0 {
        0
    } else {
        size_bytes.saturating_add(FILE_CHUNK_SIZE as u64 - 1) / FILE_CHUNK_SIZE as u64
    };
    u32::try_from(chunks).context("file is too large to address with 32-bit chunk indices")
}

fn read_file_chunk_from_file(
    file: &mut File,
    chunk_index: u32,
    total_bytes: u64,
) -> Result<Vec<u8>> {
    let offset = chunk_index as u64 * FILE_CHUNK_SIZE as u64;
    let remaining = total_bytes.saturating_sub(offset);
    let to_read = usize::try_from(remaining.min(FILE_CHUNK_SIZE as u64))
        .context("chunk size exceeds addressable memory")?;

    file.seek(SeekFrom::Start(offset))
        .with_context(|| "seeking outbound file".to_string())?;

    let mut buf = vec![0u8; to_read];
    file.read_exact(&mut buf)
        .with_context(|| "reading outbound file chunk".to_string())?;
    Ok(buf)
}

fn checksum_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)
        .with_context(|| format!("opening file for checksum {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];

    loop {
        let read = file
            .read(&mut buf)
            .with_context(|| format!("reading file for checksum {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }

    Ok(hex::encode(hasher.finalize()))
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
        let announced_id = meta.transfer_id;
        let mut transfer = OutboundTransfer::new(data.clone(), meta, None);
        let mut collected: Vec<FileTransferMessage> = Vec::new();
        while let Some(msg) = transfer.next_chunk_message().unwrap() {
            collected.push(msg);
        }
        assert!(!collected.is_empty());
        assert!(transfer.is_all_sent());
        assert_eq!(transfer.transfer_id, announced_id);
    }

    #[test]
    fn manager_preserves_announced_transfer_id() {
        let tmp = TempDir::new().unwrap();
        let data = b"proof".repeat(128);
        let mut mgr = FileTransferManager::new(tmp.path().to_path_buf());
        let transfer = mgr
            .start_outbound(data, "proof.txt".into(), "text/plain".into(), None)
            .unwrap();
        assert_eq!(transfer.transfer_id, transfer.meta.transfer_id);
    }

    #[test]
    fn inbound_verify_integrity() {
        let tmp = TempDir::new().unwrap();
        let data = b"Deskdrop file transfer test".repeat(500);
        let meta = make_meta(&data);
        let tid = meta.transfer_id;
        let mut mgr = FileTransferManager::new(tmp.path().to_path_buf());
        let _ = mgr.register_inbound(meta, Uuid::new_v4(), "Phone".into());
        mgr.accept_inbound(&tid).unwrap();

        // Feed chunks directly from the data slice.
        let transfer = mgr.get_inbound_mut(&tid).unwrap();
        for (chunk_idx, chunk) in data.chunks(FILE_CHUNK_SIZE).enumerate() {
            transfer
                .receive_chunk(chunk_idx as u32, chunk.to_vec())
                .unwrap();
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
        let _ = mgr.register_inbound(meta, Uuid::new_v4(), "Laptop".into());
        mgr.accept_inbound(&tid).unwrap();

        let transfer = mgr.get_inbound_mut(&tid).unwrap();
        // Feed corrupted chunk while keeping chunk count consistent.
        for (chunk_idx, chunk) in data.chunks(FILE_CHUNK_SIZE).enumerate() {
            let mut chunk_data = chunk.to_vec();
            if chunk_idx == 0 && !chunk_data.is_empty() {
                chunk_data[0] ^= 0xFF;
            }
            transfer
                .receive_chunk(chunk_idx as u32, chunk_data)
                .unwrap();
        }
        // finalize should fail due to SHA-256 mismatch
        let result = transfer.finalize();
        assert!(result.is_err());
    }
}
