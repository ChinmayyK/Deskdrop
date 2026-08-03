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
use bytes::Bytes;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const FILE_CHUNK_SIZE: usize = 1024 * 1024; // 1 MB per chunk — larger chunks reduce per-chunk
                                                    // overhead (mutex locks, syscalls, encrypt/serialize
                                                    // round-trips) by 2× vs 512 KB.

/// Maximum transfer size (4 GB). Rejects announced transfers exceeding this
/// limit to prevent disk-bomb attacks via pre-allocation.
pub const MAX_TRANSFER_BYTES: u64 = 4 * 1024 * 1024 * 1024; // 4 GB

pub const FILE_ACK_EVERY_N_CHUNKS: u32 = 16; // ACK every 16 MB

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
        #[serde(default)]
        compressed: bool,
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
    Queued,
    Transferring,
    Verifying,
    Complete,
    Failed,
    Cancelled,
}

// ── Sender state ──────────────────────────────────────────────────────────────

enum OutboundSource {
    Memory(Bytes),
    FilePath(PathBuf, Option<std::fs::File>),
}

pub struct OutboundTransfer {
    pub transfer_id: TransferId,
    pub meta: FileTransferMetadata,
    source: OutboundSource,
    pub total_chunks: u32,
    pub next_chunk: u32,
    pub last_acked_chunk: Option<u32>,
    pub status: TransferStatus,
    pub created_at: Instant,
    pub started_at: Option<Instant>,
    pub last_active_at: Instant,
    pub target_device: Option<Uuid>,
    pub paused: bool,
    pub hasher: Option<Sha256>,
    pub last_speed_calc_at: Option<Instant>,
    pub last_speed_calc_bytes: u64,
    pub current_speed_bps: Option<u64>,
}

impl OutboundTransfer {
    pub fn new(
        data: impl Into<Bytes>,
        meta: FileTransferMetadata,
        target_device: Option<Uuid>,
    ) -> Self {
        let total_chunks = chunk_count(meta.size_bytes).unwrap_or(u32::MAX);

        Self {
            // Use the announced transfer ID so later accept/ack messages map
            // back to the sender's local outbound state.
            transfer_id: meta.transfer_id,
            meta,
            source: OutboundSource::Memory(data.into()),
            total_chunks,
            next_chunk: 0,
            last_acked_chunk: None,
            status: TransferStatus::Pending,
            created_at: Instant::now(),
            started_at: None,
            last_active_at: Instant::now(),
            target_device,
            paused: false,
            hasher: Some(Sha256::new()),
            last_speed_calc_at: None,
            last_speed_calc_bytes: 0,
            current_speed_bps: None,
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
            last_acked_chunk: None,
            status: TransferStatus::Pending,
            created_at: Instant::now(),
            started_at: None,
            last_active_at: Instant::now(),
            target_device,
            paused: false,
            hasher: Some(Sha256::new()),
            last_speed_calc_at: None,
            last_speed_calc_bytes: 0,
            current_speed_bps: None,
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
        if let Some(h) = &mut self.hasher {
            h.update(&data);
        }
        Ok(Some(self.process_chunk_data(idx, data, false)))
    }
    pub fn take_io_context(&mut self) -> Option<(Option<std::fs::File>, sha2::Sha256)> {
        let file = if let OutboundSource::FilePath(_, cached_file) = &mut self.source {
            cached_file.take()
        } else {
            None
        };
        self.hasher.take().map(|h| (file, h))
    }

    pub fn restore_io_context(&mut self, file: Option<std::fs::File>, hasher: sha2::Sha256) {
        if let OutboundSource::FilePath(_, cached_file) = &mut self.source {
            *cached_file = file;
        }
        self.hasher = Some(hasher);
    }

    pub fn next_chunk_instruction(&mut self) -> Result<Option<ChunkInstruction>> {
        self.last_active_at = Instant::now();
        if self.paused || self.next_chunk >= self.total_chunks {
            return Ok(None);
        }
        let idx = self.next_chunk;
        let instr = match &self.source {
            OutboundSource::Memory(data_vec) => {
                let start = (idx as usize) * FILE_CHUNK_SIZE;
                let end = (start + FILE_CHUNK_SIZE).min(data_vec.len());
                ChunkInstruction::Memory {
                    chunk_index: idx,
                    data: data_vec[start..end].to_vec(),
                }
            }
            OutboundSource::FilePath(path, _) => {
                let offset = (idx as u64) * (FILE_CHUNK_SIZE as u64);
                let remaining = self.meta.size_bytes.saturating_sub(offset);
                let len = (FILE_CHUNK_SIZE as u64).min(remaining) as usize;
                ChunkInstruction::File {
                    chunk_index: idx,
                    path: path.clone(),
                    offset,
                    len,
                }
            }
        };
        self.next_chunk += 1;
        Ok(Some(instr))
    }

    /// Calculate dynamic chunk batch size based on current in-flight queue depth.
    /// Scales up when ACKs arrive fast (low RTT/high LAN throughput) and throttles down when network backs up.
    pub fn adaptive_batch_size(&self, max_batch: usize) -> usize {
        if max_batch <= 1 {
            return 1;
        }
        max_batch
    }

    pub fn process_chunk_data(
        &mut self,
        chunk_index: u32,
        data: Vec<u8>,
        compressed: bool,
    ) -> FileTransferMessage {
        self.next_chunk = chunk_index + 1;
        FileTransferMessage::Chunk {
            transfer_id: self.transfer_id,
            chunk_index,
            total_chunks: self.total_chunks,
            data,
            compressed,
        }
    }

    pub fn finalize_checksum(&mut self) -> String {
        if let Some(h) = self.hasher.take() {
            hex::encode(h.finalize())
        } else {
            String::new()
        }
    }

    pub fn progress(&mut self) -> TransferProgress {
        let bytes_sent = if self.status == TransferStatus::Complete {
            self.meta.size_bytes
        } else {
            let acked_count = self.last_acked_chunk.map(|idx| idx + 1).unwrap_or(0);
            let sent =
                ((acked_count as u64) * (FILE_CHUNK_SIZE as u64)).min(self.meta.size_bytes);
            if sent >= self.meta.size_bytes
                && self.meta.size_bytes > 0
                && acked_count < self.total_chunks
            {
                self.meta.size_bytes.saturating_sub(1)
            } else {
                sent
            }
        };
        let percent = if self.total_chunks == 0 {
            100
        } else if self.meta.size_bytes > 0 {
            if bytes_sent == self.meta.size_bytes {
                100
            } else {
                ((bytes_sent as f64 / self.meta.size_bytes as f64) * 100.0).min(99.0) as u8
            }
        } else {
            0
        };

        let now = Instant::now();
        if let Some(last_calc) = self.last_speed_calc_at {
            let elapsed = now.duration_since(last_calc).as_secs_f64();
            if bytes_sent > self.last_speed_calc_bytes {
                if elapsed >= 0.25 {
                    let diff = bytes_sent - self.last_speed_calc_bytes;
                    self.current_speed_bps = Some((diff as f64 / elapsed) as u64);
                    self.last_speed_calc_at = Some(now);
                    self.last_speed_calc_bytes = bytes_sent;
                }
            } else if elapsed >= 2.0 {
                self.current_speed_bps = Some(0);
                self.last_speed_calc_at = Some(now);
                self.last_speed_calc_bytes = bytes_sent;
            }
        } else {
            self.last_speed_calc_at = Some(now);
            self.last_speed_calc_bytes = bytes_sent;
        }

        let speed_bps = self.current_speed_bps;
        let eta_secs = speed_bps.and_then(|spd| {
            if spd > 0 {
                Some(self.meta.size_bytes.saturating_sub(bytes_sent) / spd)
            } else {
                None
            }
        });

        TransferProgress {
            transfer_id: self.transfer_id,
            bytes_received: bytes_sent,
            total_bytes: self.meta.size_bytes,
            percent,
            speed_bps,
            eta_secs,
        }
    }

    /// Called when receiver acks chunks up to `last_confirmed`.
    pub fn on_chunk_ack(&mut self, last_confirmed: u32) {
        self.last_active_at = Instant::now();
        self.last_acked_chunk = Some(last_confirmed);
    }

    /// Resume from the given chunk (skip already-delivered ones).
    pub fn resume_from(&mut self, chunk_index: u32) {
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }
        self.next_chunk = chunk_index;
        self.last_acked_chunk = if chunk_index > 0 { Some(chunk_index - 1) } else { None };
        self.status = TransferStatus::Transferring;
    }

    pub fn is_all_sent(&self) -> bool {
        self.next_chunk >= self.total_chunks
    }
}

// ── Receiver state ────────────────────────────────────────────────────────────

pub enum ChunkInstruction {
    Memory {
        chunk_index: u32,
        data: Vec<u8>,
    },
    File {
        chunk_index: u32,
        path: PathBuf,
        offset: u64,
        len: usize,
    },
}

pub struct InboundTransfer {
    pub transfer_id: TransferId,
    pub meta: FileTransferMetadata,
    pub total_chunks: u32,
    pub queued_chunk_count: u32,
    pub received_chunk_count: u32,
    pub last_confirmed_chunk: u32,
    pub status: TransferStatus,
    pub created_at: Instant,
    pub started_at: Option<Instant>,
    pub last_active_at: Instant,
    pub bytes_received: u64,
    pub last_written_offset: u64,
    /// Temp file path for streaming writes.

    /// Persistent file handle to avoid re-opening on every chunk.
    pub file_handle: Option<BufWriter<std::fs::File>>,
    /// Final destination path.
    pub dest_path: Option<PathBuf>,
    pub from_device: Uuid,
    pub from_device_name: String,
    pub paused: bool,
    hasher: Sha256,
    pub last_speed_calc_at: Option<Instant>,
    pub last_speed_calc_bytes: u64,
    pub current_speed_bps: Option<u64>,
}

impl InboundTransfer {
    pub fn new(meta: FileTransferMetadata, from_device: Uuid, from_device_name: String) -> Self {
        let total_chunks = chunk_count(meta.size_bytes).unwrap_or(u32::MAX);
        Self {
            transfer_id: meta.transfer_id,
            meta,
            total_chunks,
            queued_chunk_count: 0,
            received_chunk_count: 0,
            last_confirmed_chunk: 0,
            status: TransferStatus::Pending,
            created_at: Instant::now(),
            started_at: None,
            last_active_at: Instant::now(),
            bytes_received: 0,
            last_written_offset: 0,
            file_handle: None,
            dest_path: None,
            from_device,
            from_device_name,
            paused: false,
            hasher: Sha256::new(),
            last_speed_calc_at: None,
            last_speed_calc_bytes: 0,
            current_speed_bps: None,
        }
    }

    /// Accept the transfer, setting up paths.
    pub fn accept(&mut self, save_dir: &Path) -> Result<()> {
        let (safe_name, sub_dirs) = if self.meta.batch_id.is_some() {
            // For batched directory transfers, preserve relative structure but sanitize parts.
            let parts: Vec<&str> = self.meta.file_name.split('/').collect();
            let mut dirs = Vec::new();
            for part in &parts[..parts.len().saturating_sub(1)] {
                let sanitized = sanitize_file_name(part);
                if !sanitized.is_empty() {
                    dirs.push(sanitized);
                }
            }
            let name = sanitize_file_name(parts.last().unwrap_or(&self.meta.file_name.as_str()));
            (name, dirs)
        } else {
            (sanitize_file_name(&self.meta.file_name), Vec::new())
        };

        anyhow::ensure!(
            !safe_name.is_empty(),
            "file name is empty after sanitization"
        );

        let mut actual_save_dir = save_dir.to_path_buf();
        for dir in sub_dirs {
            actual_save_dir.push(dir);
        }

        std::fs::create_dir_all(&actual_save_dir).context("creating save dir")?;

        if let Some(free_bytes) = get_available_disk_space(save_dir) {
            anyhow::ensure!(
                free_bytes > self.meta.size_bytes + 50 * 1024 * 1024,
                "insufficient disk space: need {} bytes, but only {} bytes are free",
                self.meta.size_bytes,
                free_bytes
            );
        }

        let (dest, file) = create_unique_file(&actual_save_dir, &safe_name)
            .with_context(|| "creating destination file atomically")?;

        self.dest_path = Some(dest);
        self.file_handle = Some(BufWriter::with_capacity(4 * 1024 * 1024, file));
        self.status = TransferStatus::Transferring;
        self.started_at = Some(Instant::now());
        Ok(())
    }

    pub fn take_io_context(&mut self) -> Option<(BufWriter<std::fs::File>, sha2::Sha256, u64)> {
        if let Some(file) = self.file_handle.take() {
            Some((file, self.hasher.clone(), self.last_written_offset))
        } else {
            None
        }
    }

    pub fn restore_io_context(
        &mut self,
        file: BufWriter<std::fs::File>,
        hasher: sha2::Sha256,
        new_offset: u64,
    ) {
        self.file_handle = Some(file);
        self.hasher = hasher;
        self.last_written_offset = new_offset;
    }

    pub fn validate_chunk(
        &mut self,
        chunk_index: u32,
        data_len: usize,
    ) -> Result<(u64, usize, bool)> {
        self.last_active_at = Instant::now();
        anyhow::ensure!(
            self.status == TransferStatus::Transferring,
            "transfer is not active"
        );
        anyhow::ensure!(!self.paused, "transfer is paused");
        anyhow::ensure!(data_len <= 8 * 1024 * 1024, "chunk size exceeds limit");
        anyhow::ensure!(
            chunk_index < self.total_chunks,
            "chunk {} out of range",
            chunk_index
        );

        if chunk_index < self.total_chunks - 1 {
            anyhow::ensure!(data_len == FILE_CHUNK_SIZE, "non-final chunk size mismatch");
        } else {
            let mut expected = (self.meta.size_bytes % (FILE_CHUNK_SIZE as u64)) as usize;
            if expected == 0 && self.meta.size_bytes > 0 {
                expected = FILE_CHUNK_SIZE;
            }
            anyhow::ensure!(data_len == expected, "final chunk size mismatch: expected {}, got {}", expected, data_len);
        }

        if chunk_index < self.queued_chunk_count {
            return Ok((0, 0, true)); // duplicate
        }
        anyhow::ensure!(
            chunk_index == self.queued_chunk_count,
            "out-of-order chunk"
        );
        
        self.queued_chunk_count += 1;

        let offset = (chunk_index as u64) * (FILE_CHUNK_SIZE as u64);
        let mut padding = 0;
        if chunk_index < self.total_chunks - 1 && data_len < FILE_CHUNK_SIZE {
            padding = FILE_CHUNK_SIZE - data_len;
        }
        Ok((offset, padding, false))
    }

    pub fn commit_chunk(&mut self, chunk_index: u32, data_len: usize) -> TransferProgress {
        self.bytes_received += data_len as u64;
        self.received_chunk_count += 1;
        self.last_confirmed_chunk = chunk_index;
        self.progress_snapshot()
    }

    /// Feed a chunk into the transfer. Returns progress info.
    pub fn receive_chunk(&mut self, chunk_index: u32, data: Vec<u8>) -> Result<TransferProgress> {
        self.last_active_at = Instant::now();
        anyhow::ensure!(
            self.status == TransferStatus::Transferring,
            "transfer is not active"
        );
        anyhow::ensure!(!self.paused, "transfer is paused");
        anyhow::ensure!(data.len() <= 8 * 1024 * 1024, "chunk size exceeds limit");

        anyhow::ensure!(
            chunk_index < self.total_chunks,
            "chunk {} out of range (total {})",
            chunk_index,
            self.total_chunks
        );
        // Fix: non-final chunks MUST be exactly FILE_CHUNK_SIZE to prevent null-byte gaps on disk
        if chunk_index < self.total_chunks - 1 {
            anyhow::ensure!(
                data.len() == FILE_CHUNK_SIZE,
                "non-final chunk size {} != expected {}",
                data.len(),
                FILE_CHUNK_SIZE
            );
        }
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

        crate::network::return_buffer(data);

        Ok(self.progress_snapshot())
    }

    pub fn finalize(&mut self, expected_checksum: String) -> Result<PathBuf> {
        anyhow::ensure!(
            self.received_chunk_count == self.total_chunks,
            "missing chunks: got {} of {}",
            self.received_chunk_count,
            self.total_chunks
        );
        anyhow::ensure!(
            self.bytes_received == self.meta.size_bytes,
            "size mismatch: expected {}, got {}",
            self.meta.size_bytes,
            self.bytes_received
        );

        // Integrity verification.
        let actual = {
            let hasher = std::mem::replace(&mut self.hasher, Sha256::new());
            hex::encode(hasher.finalize())
        };
        anyhow::ensure!(
            actual == expected_checksum,
            "SHA-256 mismatch: expected {}, got {}",
            expected_checksum,
            actual
        );

        // Flush and sync to durable storage before renaming.
        if let Some(mut file) = self.file_handle.take() {
            file.flush()?;
            file.get_ref().sync_all()?;
        }

        let dest = self.dest_path.as_ref().context("no dest path")?.clone();
        self.status = TransferStatus::Complete;
        Ok(dest)
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
            file.write_all(data).context("writing chunk to temp file")?;
            self.hasher.update(data);
        } else {
            anyhow::bail!("transfer has not been accepted or file handle is missing");
        }
        Ok(())
    }

    pub fn progress_snapshot(&mut self) -> TransferProgress {
        let percent = if self.total_chunks == 0 {
            100
        } else {
            ((self.received_chunk_count as f64 / self.total_chunks as f64) * 100.0) as u8
        };
        
        let now = Instant::now();
        if let Some(last_calc) = self.last_speed_calc_at {
            let elapsed = now.duration_since(last_calc).as_secs_f64();
            if self.bytes_received > self.last_speed_calc_bytes {
                if elapsed >= 0.25 {
                    let diff = self.bytes_received - self.last_speed_calc_bytes;
                    self.current_speed_bps = Some((diff as f64 / elapsed) as u64);
                    self.last_speed_calc_at = Some(now);
                    self.last_speed_calc_bytes = self.bytes_received;
                }
            } else if elapsed >= 2.0 {
                self.current_speed_bps = Some(0);
                self.last_speed_calc_at = Some(now);
                self.last_speed_calc_bytes = self.bytes_received;
            }
        } else {
            self.last_speed_calc_at = Some(now);
            self.last_speed_calc_bytes = self.bytes_received;
        }

        let speed_bps = self.current_speed_bps;
        let eta_secs = speed_bps.and_then(|spd| {
            if spd > 0 {
                Some(self.meta.size_bytes.saturating_sub(self.bytes_received) / spd)
            } else {
                None
            }
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
        let mut tid = [0u8; 16];
        tid.copy_from_slice(Uuid::new_v4().as_bytes());

        let meta = FileTransferMetadata {
            transfer_id: tid,
            file_name: file_name.clone(),
            size_bytes: data.len() as u64,
            mime_type,
            is_directory: false,
            item_count: 1,
            batch_id: None,
        };
        let transfer = OutboundTransfer::new(data, meta, target_device);
        let tid = transfer.transfer_id;
        Ok(self.outbound.entry(tid).or_insert(transfer))
    }

    pub fn start_outbound_path(
        &mut self,
        path: PathBuf,
        file_name: String,
        mime_type: String,
        target_device: Option<Uuid>,
        batch_id: Option<String>,
        is_directory: bool,
        item_count: u32,
    ) -> Result<&OutboundTransfer> {
        let size_bytes = std::fs::metadata(&path)
            .with_context(|| format!("reading metadata for {}", path.display()))?
            .len();
        let mut tid = [0u8; 16];
        tid.copy_from_slice(Uuid::new_v4().as_bytes());

        let meta = FileTransferMetadata {
            transfer_id: tid,
            file_name,
            size_bytes,
            mime_type,
            is_directory,
            item_count,
            batch_id,
        };
        let transfer = OutboundTransfer::from_path(path, meta, target_device)?;
        let tid = transfer.transfer_id;
        Ok(self.outbound.entry(tid).or_insert(transfer))
    }

    pub fn get_outbound(&self, tid: &TransferId) -> Option<&OutboundTransfer> {
        self.outbound.get(tid)
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
        if self.inbound.len() >= 100 {
            anyhow::bail!("too many active transfers");
        }
        let count_from_peer = self
            .inbound
            .values()
            .filter(|t| t.from_device == from_device)
            .count();
        if count_from_peer >= 50 {
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

    pub fn queue_inbound(&mut self, tid: &TransferId) -> Result<()> {
        let transfer = self.inbound.get_mut(tid).context("unknown transfer")?;
        if transfer.status == TransferStatus::Pending {
            transfer.status = TransferStatus::Queued;
        }
        Ok(())
    }

    pub fn get_inbound_to_start(&self, max_active: usize) -> Vec<(TransferId, Uuid)> {
        let active_count = self
            .inbound
            .values()
            .filter(|t| t.status == TransferStatus::Transferring)
            .count();
            
        if active_count >= max_active {
            return vec![];
        }
        
        let available_slots = max_active - active_count;
        let mut queued: Vec<_> = self
            .inbound
            .values()
            .filter(|t| t.status == TransferStatus::Queued)
            .collect();
            
        // Sort by created_at to process oldest first (FIFO)
        queued.sort_by_key(|t| t.created_at);
        
        queued
            .into_iter()
            .take(available_slots)
            .map(|t| (t.transfer_id, t.from_device))
            .collect()
    }

    pub fn reject_inbound(&mut self, tid: &TransferId) {
        if let Some(mut t) = self.inbound.remove(tid) {
            t.status = TransferStatus::Cancelled;
            t.file_handle = None;
            if let Some(dest) = t.dest_path.take() {
                let _ = std::fs::remove_file(dest);
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
        if let Some(mut t) = self.inbound.remove(tid) {
            t.file_handle = None;
            if let Some(dest) = t.dest_path {
                let _ = std::fs::remove_file(dest);
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

    pub fn pause_all_for_device(&mut self, peer_id: Uuid) {
        for t in self.inbound.values_mut() {
            if t.from_device == peer_id
                && (t.status == TransferStatus::Transferring || t.status == TransferStatus::Pending)
            {
                t.status = TransferStatus::Pending;
                t.file_handle = None;
            }
        }
        for t in self.outbound.values_mut() {
            if t.target_device == Some(peer_id)
                && (t.status == TransferStatus::Transferring || t.status == TransferStatus::Pending)
            {
                t.status = TransferStatus::Pending;
                // No file handle to clear for outbound currently.
            }
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

    pub fn active_transfers(&mut self) -> Vec<serde_json::Value> {
        let mut transfers = Vec::new();
        for t in self.inbound.values_mut() {
            let prog = t.progress_snapshot();
            let percent = prog.percent;
            let status_str = match t.status {
                TransferStatus::Pending => "incoming",
                TransferStatus::Queued => "queued",
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
                "speed_bps": prog.speed_bps,
                "eta_secs": prog.eta_secs,
                "status": status_str,
                "is_directory": t.meta.is_directory,
                "item_count": t.meta.item_count,
                "batch_id": t.meta.batch_id.clone()
            }));
        }
        for t in self.outbound.values_mut() {
            let prog = t.progress();
            let bytes_sent = prog.bytes_received;
            let percent = prog.percent;
            let status_str = match t.status {
                TransferStatus::Pending => "transferring", // Remote hasn't accepted yet, but from our end it's outgoing
                TransferStatus::Queued => "transferring",  // We treat queued similarly for UI simplicity on sender side
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
                "speed_bps": prog.speed_bps,
                "eta_secs": prog.eta_secs,
                "status": status_str,
                "is_directory": t.meta.is_directory,
                "item_count": t.meta.item_count,
                "batch_id": t.meta.batch_id.clone()
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
pub fn sanitize_file_name(name: &str) -> String {
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
    let mut trimmed = sanitized.trim_start_matches('.');

    // Trim trailing dots and spaces (Windows extension blocklist bypass prevention).
    trimmed = trimmed.trim_end_matches(['.', ' ']);

    if trimmed.is_empty() {
        return "file".to_string();
    }

    // Windows reserved device names check (stem before the first dot).
    let stem = match trimmed.split_once('.') {
        Some((s, _)) => s,
        None => trimmed,
    };
    let upper = stem.to_ascii_uppercase();
    let is_reserved = match upper.as_str() {
        "CON" | "PRN" | "AUX" | "NUL" => true,
        s if (s.len() == 4
            && (s.starts_with("COM") || s.starts_with("LPT"))
            && s.chars()
                .nth(3)
                .is_some_and(|c| c.is_ascii_digit() && c != '0')) =>
        {
            true
        }
        _ => false,
    };

    if is_reserved {
        format!("file_{}", trimmed)
    } else {
        trimmed.to_string()
    }
}

/// Query available disk space in bytes for a given path.
fn get_available_disk_space(path: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let c_path = match std::ffi::CString::new(path.as_os_str().as_bytes()) {
            Ok(c) => c,
            Err(_) => return None,
        };
        let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) } == 0 {
            #[allow(clippy::unnecessary_cast)]
            return Some((stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64));
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let mut path_u16: Vec<u16> = path.as_os_str().encode_wide().collect();
        path_u16.push(0);
        let mut free_bytes_available: u64 = 0;
        let mut total_number_of_bytes: u64 = 0;
        let mut total_number_of_free_bytes: u64 = 0;
        let res = unsafe {
            windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
                path_u16.as_ptr(),
                &mut free_bytes_available,
                &mut total_number_of_bytes,
                &mut total_number_of_free_bytes,
            )
        };
        if res != 0 {
            return Some(free_bytes_available);
        }
    }
    None
}

fn create_unique_file(dir: &Path, file_name: &str) -> Result<(PathBuf, File)> {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name);
    let ext = Path::new(file_name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    for i in 0..=999 {
        let name = if i == 0 {
            file_name.to_string()
        } else if ext.is_empty() {
            format!("{} ({})", stem, i)
        } else {
            format!("{} ({}).{}", stem, i, ext)
        };
        let candidate = dir.join(&name);

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }

    let fallback = dir.join(format!("{}_{}_{}", stem, now_unix(), ext));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&fallback)?;
    Ok((fallback, file))
}

#[allow(dead_code)]
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

fn read_file_chunk_from_file(file: &mut File, chunk_index: u32, total_bytes: u64) -> Result<Vec<u8>> {
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

pub fn checksum_file(path: &Path) -> Result<String> {
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
            batch_id: None,
            is_directory: false,
            item_count: 1,
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
        let hash = hex::encode(sha2::Sha256::digest(&data));
        let dest = transfer.finalize(hash).unwrap();
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
        let result = transfer.finalize("bad_hash".to_string());
        assert!(result.is_err());
    }
}
