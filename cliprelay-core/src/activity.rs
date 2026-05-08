//! ClipRelay Activity Feed — unified cross-device event log.
//!
//! Every meaningful event across all connected devices is recorded here:
//!   - Clipboard copies (from any device)
//!   - File transfers (sent/received)
//!   - Device connect/disconnect
//!   - Sync pause/resume
//!
//! The feed drives the Timeline-First clipboard UX: remote clipboard items
//! land here first, and users explicitly "apply" them rather than having
//! their local clipboard overwritten automatically.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ── Activity kinds ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    /// A device copied text to its clipboard.
    ClipboardText,
    /// A device copied an image.
    ClipboardImage,
    /// A file transfer was initiated.
    FileTransferStarted,
    /// A file transfer completed successfully.
    FileTransferComplete,
    /// A file transfer was cancelled or failed.
    FileTransferFailed,
    /// A peer connected to the mesh.
    PeerConnected,
    /// A peer disconnected from the mesh.
    PeerDisconnected,
    /// Sync was paused for a peer.
    SyncPaused,
    /// Sync was resumed for a peer.
    SyncResumed,
    /// A clipboard item from a remote device is available to apply locally.
    RemoteClipboardAvailable,
    /// User explicitly applied a remote clipboard item.
    ClipboardApplied,
}

/// A single entry in the activity feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    /// Monotonically incrementing ID within this session.
    pub id: u64,
    /// Milliseconds since Unix epoch.
    pub timestamp_ms: u64,
    /// Device that originated this event.
    pub device_id: Uuid,
    /// Human-readable device name (never a raw UUID in the UI).
    pub device_name: String,
    /// What kind of event this is.
    pub kind: ActivityKind,
    /// Short human-readable description for the feed.
    pub summary: String,
    /// Optional content hash (for clipboard items, used for dedup + apply).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// For clipboard items: the text preview (truncated).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_preview: Option<String>,
    /// For file transfers: file name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// For file transfers: total bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_bytes: Option<u64>,
    /// Transfer ID for file events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transfer_id: Option<String>,
    /// Whether this item has been applied locally.
    pub applied_locally: bool,
    /// Relay path: which devices forwarded this event (mesh traceability).
    pub relay_path: Vec<String>,
}

impl ActivityEntry {
    fn new(id: u64, device_id: Uuid, device_name: String, kind: ActivityKind, summary: String) -> Self {
        Self {
            id,
            timestamp_ms: now_ms(),
            device_id,
            device_name,
            kind,
            summary,
            content_hash: None,
            text_preview: None,
            file_name: None,
            file_bytes: None,
            transfer_id: None,
            applied_locally: false,
            relay_path: Vec::new(),
        }
    }
}

// ── ActivityFeed ──────────────────────────────────────────────────────────────

/// Bounded ring-buffer activity feed.
pub struct ActivityFeed {
    entries: VecDeque<ActivityEntry>,
    capacity: usize,
    next_id: u64,
}

impl ActivityFeed {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
            next_id: 1,
        }
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn push(&mut self, entry: ActivityEntry) {
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    // ── Constructors for each event type ─────────────────────────────────────

    /// Record a remote clipboard text item landing in the feed.
    pub fn record_remote_clipboard_text(
        &mut self,
        device_id: Uuid,
        device_name: String,
        text: &str,
        content_hash: String,
        relay_path: Vec<String>,
    ) -> u64 {
        let preview = if text.len() > 80 {
            format!("{}…", &text[..80])
        } else {
            text.to_string()
        };
        let summary = format!("[{}] copied text: {}", device_name, &preview[..preview.len().min(40)]);
        let id = self.alloc_id();
        let mut entry = ActivityEntry::new(id, device_id, device_name, ActivityKind::RemoteClipboardAvailable, summary);
        entry.content_hash = Some(content_hash);
        entry.text_preview = Some(preview);
        entry.relay_path = relay_path;
        self.push(entry);
        id
    }

    /// Record a remote clipboard image item.
    pub fn record_remote_clipboard_image(
        &mut self,
        device_id: Uuid,
        device_name: String,
        mime: &str,
        bytes: u64,
        content_hash: String,
        relay_path: Vec<String>,
    ) -> u64 {
        let summary = format!("[{}] copied image ({}, {} KB)", device_name, mime, bytes / 1024);
        let id = self.alloc_id();
        let mut entry = ActivityEntry::new(id, device_id, device_name, ActivityKind::RemoteClipboardAvailable, summary);
        entry.content_hash = Some(content_hash);
        entry.file_bytes = Some(bytes);
        entry.relay_path = relay_path;
        self.push(entry);
        id
    }

    /// Record that a remote clipboard item was applied to local clipboard.
    pub fn record_clipboard_applied(&mut self, device_id: Uuid, device_name: String, content_hash: String) -> u64 {
        // Mark the original "available" entry as applied.
        for e in self.entries.iter_mut() {
            if e.content_hash.as_deref() == Some(&content_hash) && e.kind == ActivityKind::RemoteClipboardAvailable {
                e.applied_locally = true;
            }
        }
        let summary = format!("Applied clipboard from [{}]", device_name);
        let id = self.alloc_id();
        let mut entry = ActivityEntry::new(id, device_id, device_name, ActivityKind::ClipboardApplied, summary);
        entry.content_hash = Some(content_hash);
        self.push(entry);
        id
    }

    /// Record a local clipboard copy (sent to peers).
    pub fn record_local_clipboard_text(&mut self, device_id: Uuid, device_name: String, text: &str, content_hash: String) -> u64 {
        let preview = if text.len() > 40 { format!("{}…", &text[..40]) } else { text.to_string() };
        let summary = format!("[{}] copied text", device_name);
        let id = self.alloc_id();
        let mut entry = ActivityEntry::new(id, device_id, device_name, ActivityKind::ClipboardText, summary);
        entry.content_hash = Some(content_hash);
        entry.text_preview = Some(preview);
        self.push(entry);
        id
    }

    /// Record file transfer start.
    pub fn record_file_transfer_started(
        &mut self,
        device_id: Uuid,
        device_name: String,
        file_name: String,
        file_bytes: u64,
        transfer_id: String,
        is_sender: bool,
    ) -> u64 {
        let summary = if is_sender {
            format!("[{}] sending file: {}", device_name, file_name)
        } else {
            format!("[{}] receiving file: {}", device_name, file_name)
        };
        let id = self.alloc_id();
        let mut entry = ActivityEntry::new(id, device_id, device_name, ActivityKind::FileTransferStarted, summary);
        entry.file_name = Some(file_name);
        entry.file_bytes = Some(file_bytes);
        entry.transfer_id = Some(transfer_id);
        self.push(entry);
        id
    }

    /// Record file transfer completion.
    pub fn record_file_transfer_complete(
        &mut self,
        device_id: Uuid,
        device_name: String,
        file_name: String,
        file_bytes: u64,
        transfer_id: String,
    ) -> u64 {
        // Mark the corresponding FileTransferStarted entry.
        for e in self.entries.iter_mut() {
            if e.transfer_id.as_deref() == Some(&transfer_id) && e.kind == ActivityKind::FileTransferStarted {
                e.kind = ActivityKind::FileTransferComplete;
                e.summary = format!("[{}] received file: {} ({} KB)", device_name, file_name, file_bytes / 1024);
            }
        }
        let summary = format!("[{}] file ready: {} ({} KB)", device_name, file_name, file_bytes / 1024);
        let id = self.alloc_id();
        let mut entry = ActivityEntry::new(id, device_id, device_name, ActivityKind::FileTransferComplete, summary);
        entry.file_name = Some(file_name);
        entry.file_bytes = Some(file_bytes);
        entry.transfer_id = Some(transfer_id);
        self.push(entry);
        id
    }

    /// Record file transfer failure.
    pub fn record_file_transfer_failed(
        &mut self,
        device_id: Uuid,
        device_name: String,
        file_name: Option<String>,
        transfer_id: String,
        reason: String,
    ) -> u64 {
        for e in self.entries.iter_mut() {
            if e.transfer_id.as_deref() == Some(&transfer_id) {
                e.kind = ActivityKind::FileTransferFailed;
            }
        }
        let name_part = file_name.as_deref().unwrap_or("unknown file");
        let summary = format!("[{}] transfer failed: {} — {}", device_name, name_part, reason);
        let id = self.alloc_id();
        let mut entry = ActivityEntry::new(id, device_id, device_name, ActivityKind::FileTransferFailed, summary);
        entry.file_name = file_name;
        entry.transfer_id = Some(transfer_id);
        self.push(entry);
        id
    }

    /// Record a peer connecting.
    pub fn record_peer_connected(&mut self, device_id: Uuid, device_name: String) -> u64 {
        let summary = format!("[{}] connected", device_name);
        let id = self.alloc_id();
        let entry = ActivityEntry::new(id, device_id, device_name, ActivityKind::PeerConnected, summary);
        self.push(entry);
        id
    }

    /// Record a peer disconnecting.
    pub fn record_peer_disconnected(&mut self, device_id: Uuid, device_name: String, reason: Option<String>) -> u64 {
        let summary = match &reason {
            Some(r) => format!("[{}] disconnected: {}", device_name, r),
            None => format!("[{}] disconnected", device_name),
        };
        let id = self.alloc_id();
        let entry = ActivityEntry::new(id, device_id, device_name, ActivityKind::PeerDisconnected, summary);
        self.push(entry);
        id
    }

    /// Record sync pause.
    pub fn record_sync_paused(&mut self, device_id: Uuid, device_name: String) -> u64 {
        let summary = format!("[{}] sync paused", device_name);
        let id = self.alloc_id();
        let entry = ActivityEntry::new(id, device_id, device_name, ActivityKind::SyncPaused, summary);
        self.push(entry);
        id
    }

    /// Record sync resume.
    pub fn record_sync_resumed(&mut self, device_id: Uuid, device_name: String) -> u64 {
        let summary = format!("[{}] sync resumed", device_name);
        let id = self.alloc_id();
        let entry = ActivityEntry::new(id, device_id, device_name, ActivityKind::SyncResumed, summary);
        self.push(entry);
        id
    }

    // ── Query ─────────────────────────────────────────────────────────────────

    pub fn recent(&self, limit: usize) -> Vec<&ActivityEntry> {
        self.entries.iter().rev().take(limit).collect()
    }

    pub fn since(&self, since_id: u64) -> Vec<&ActivityEntry> {
        self.entries.iter().filter(|e| e.id > since_id).collect()
    }

    pub fn all(&self) -> Vec<&ActivityEntry> {
        self.entries.iter().collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get pending (not yet locally applied) remote clipboard items.
    pub fn pending_remote_clipboards(&self) -> Vec<&ActivityEntry> {
        self.entries
            .iter()
            .filter(|e| e.kind == ActivityKind::RemoteClipboardAvailable && !e.applied_locally)
            .collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for ActivityFeed {
    fn default() -> Self {
        Self::new(200)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_records_and_queries() {
        let mut feed = ActivityFeed::new(50);
        let id = Uuid::new_v4();
        feed.record_peer_connected(id, "Pixel 8".into());
        feed.record_remote_clipboard_text(id, "Pixel 8".into(), "hello world", "abc123".into(), vec![]);
        assert_eq!(feed.len(), 2);
        let pending = feed.pending_remote_clipboards();
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn apply_marks_entry() {
        let mut feed = ActivityFeed::new(50);
        let id = Uuid::new_v4();
        feed.record_remote_clipboard_text(id, "Phone".into(), "text", "hash1".into(), vec![]);
        feed.record_clipboard_applied(id, "Phone".into(), "hash1".into());
        assert_eq!(feed.pending_remote_clipboards().len(), 0);
    }

    #[test]
    fn capacity_evicts_oldest() {
        let mut feed = ActivityFeed::new(3);
        let id = Uuid::new_v4();
        for i in 0..5 {
            feed.record_peer_connected(id, format!("Device {}", i));
        }
        assert_eq!(feed.len(), 3);
    }
}
