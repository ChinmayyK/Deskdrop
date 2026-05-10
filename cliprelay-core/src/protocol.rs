//! ClipRelay Protocol — wire format definitions
//!
//! All messages are length-prefixed (u32 LE) + bincode-encoded.
//! After the handshake, every frame is AEAD-encrypted with a
//! per-session ChaCha20-Poly1305 key derived via X25519 ECDH + HKDF.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024; // 4 MB
pub const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024; // 32 MB
pub const MAX_FILE_BYTES: usize = 512 * 1024 * 1024; // 512 MB (chunked)

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClipboardContent {
    Text(String),
    Image {
        mime: String,
        data: Vec<u8>,
    },
    /// File payload — delivered as clipboard and also saved to Downloads/ClipRelay.
    File {
        name: String,
        data: Vec<u8>,
    },
}

impl ClipboardContent {
    pub fn byte_len(&self) -> usize {
        match self {
            ClipboardContent::Text(s) => s.len(),
            ClipboardContent::Image { data, .. } => data.len(),
            ClipboardContent::File { data, .. } => data.len(),
        }
    }

    /// Fix 16: Returns true if the content carries no actual data.
    ///
    /// Previously every call site re-implemented this guard:
    /// ```ignore
    /// if matches!(&content, ClipboardContent::Text(s) if s.is_empty()) { ... }
    /// ```
    /// Now the engine can do a single `if content.is_empty() { return; }` check
    /// before broadcasting, preventing empty clipboard events from propagating.
    pub fn is_empty(&self) -> bool {
        match self {
            ClipboardContent::Text(s) => s.is_empty(),
            ClipboardContent::Image { data, .. } => data.is_empty(),
            ClipboardContent::File { data, .. } => data.is_empty(),
        }
    }

    pub fn kind_str(&self) -> &'static str {
        match self {
            ClipboardContent::Text(_) => "text",
            ClipboardContent::Image { .. } => "image",
            ClipboardContent::File { .. } => "file",
        }
    }

    /// A short human-readable preview suitable for notifications and timeline entries.
    ///
    /// Text is truncated to `max_chars` with an ellipsis; images and files show
    /// their type and size. The preview is always a single line.
    pub fn truncated_preview(&self, max_chars: usize) -> String {
        match self {
            ClipboardContent::Text(s) => {
                // Collapse to first non-empty line, then truncate.
                let first = s
                    .lines()
                    .map(str::trim)
                    .find(|l| !l.is_empty())
                    .unwrap_or("(empty)");
                if first.len() <= max_chars {
                    first.to_string()
                } else {
                    // Truncate at a char boundary.
                    let mut end = max_chars.saturating_sub(1);
                    while end > 0 && !first.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!("{}…", &first[..end])
                }
            }
            ClipboardContent::Image { mime, data } => {
                let kb = data.len() as f64 / 1024.0;
                if kb >= 1024.0 {
                    format!("[Image {} {:.1} MB]", mime, kb / 1024.0)
                } else {
                    format!("[Image {} {:.0} KB]", mime, kb)
                }
            }
            ClipboardContent::File { name, data } => {
                let kb = data.len() as f64 / 1024.0;
                if kb >= 1024.0 {
                    format!("[File '{}' {:.1} MB]", name, kb / 1024.0)
                } else {
                    format!("[File '{}' {:.0} KB]", name, kb)
                }
            }
        }
    }

    /// Word count for text content; 0 for images and files.
    pub fn word_count(&self) -> usize {
        match self {
            ClipboardContent::Text(s) => s.split_whitespace().count(),
            _ => 0,
        }
    }

    /// Line count for text content; 0 for images and files.
    pub fn line_count(&self) -> usize {
        match self {
            ClipboardContent::Text(s) => s.lines().count(),
            _ => 0,
        }
    }
}

// ── History metadata ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryMetadata {
    pub hash: String,
    pub timestamp: u64,
    /// Human-readable device name — NEVER a raw UUID.
    /// Example: "Chinmay's Pixel 8", "MacBook Pro"
    pub source_device: String,
    pub kind: String,
    pub bytes: u64,
    pub pinned: bool,
}

impl HistoryMetadata {
    pub fn from_content(content: &ClipboardContent, source_device: String, pinned: bool) -> Self {
        let hash = hex::encode(crate::dedup::hash_content(content));
        let (kind, bytes) = match content {
            ClipboardContent::Text(text) => ("text".to_string(), text.len() as u64),
            ClipboardContent::Image { data, .. } => ("image".to_string(), data.len() as u64),
            ClipboardContent::File { data, .. } => ("file".to_string(), data.len() as u64),
        };
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            hash,
            timestamp,
            source_device,
            kind,
            bytes,
            pinned,
        }
    }

    /// Human-readable summary shown in timeline.
    /// Uses friendly device name, NOT internal ID.
    pub fn summary(&self) -> String {
        match self.kind.as_str() {
            "text" => format!("[{}] copied text", self.source_device),
            "image" => format!("[{}] copied image", self.source_device),
            "file" => format!("[{}] received file", self.source_device),
            _ => format!("[{}] clipboard item", self.source_device),
        }
    }
}

// ── Device metadata (exchanged during handshake / discovery) ─────────────────

/// Sent in the HelloFrame `device_name` extension slot so peers know
/// the platform and can format notifications like "📱 Copied from Chinmay's Pixel 8".
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeviceMetadata {
    /// User-visible name: "Chinmay's Pixel 8", "MacBook Pro", "DESKTOP-ABC123"
    pub device_name: String,
    /// "Android", "macOS", "Windows", "Linux"
    pub platform: String,
    /// OS version string (best-effort)
    pub platform_version: String,
    /// ClipRelay app version
    pub app_version: String,
}

// ── File transfer metadata ────────────────────────────────────────────────────

/// Announced before a chunked file transfer begins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferMetadata {
    pub transfer_id: [u8; 16],
    pub file_name: String,
    pub size_bytes: u64,
    pub mime_type: String,
    /// SHA-256 checksum of the complete file (hex-encoded).
    pub sha256_checksum: String,
}

// ── Wire messages ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloFrame {
    pub version: u16,
    pub device_id: Uuid,
    pub device_name: String,
    pub identity_pubkey: [u8; 32],
    pub ecdh_pubkey: [u8; 32],
    pub nonce: [u8; 16],
    /// Optional structured platform metadata (encoded as JSON string).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAckFrame {
    pub version: u16,
    pub device_id: Uuid,
    pub device_name: String,
    pub identity_pubkey: [u8; 32],
    pub ecdh_pubkey: [u8; 32],
    pub nonce_response: [u8; 16],
    pub trusted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppMessage {
    ClipboardPush {
        seq: u64,
        content: ClipboardContent,
        origin_device: Uuid,
        /// Friendly name of the originating device for UI display.
        /// Never a raw UUID.
        origin_device_name: String,
        /// Relay path for mesh tracing: list of device names that forwarded this.
        #[serde(default)]
        relay_path: Vec<String>,
    },
    HistoryMetadata {
        entry: HistoryMetadata,
    },
    ClipboardAck {
        seq: u64,
    },
    /// Announce a file transfer before sending chunks (dedicated pipeline).
    FileTransferAnnounce {
        meta: FileTransferMetadata,
    },
    /// Receiver accepts, rejects, or resumes a transfer.
    FileTransferAccept {
        transfer_id: [u8; 16],
        accepted: bool,
        /// Resume from this chunk index (0 = fresh start).
        #[serde(default)]
        resume_from_chunk: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        reject_reason: Option<String>,
    },
    /// One chunk of file data (dedicated file transfer channel).
    FileChunk {
        transfer_id: [u8; 16],
        chunk_index: u32,
        total_chunks: u32,
        data: Vec<u8>,
    },
    /// Periodic acknowledgement from receiver → sender.
    FileChunkAck {
        transfer_id: [u8; 16],
        last_confirmed_chunk: u32,
    },
    /// Sender signals all chunks sent; receiver should verify.
    FileTransferComplete {
        transfer_id: [u8; 16],
    },
    /// Receiver confirms finalization (success or error).
    FileTransferCompleteAck {
        transfer_id: [u8; 16],
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// Either side cancels a transfer in progress.
    FileTransferCancel {
        transfer_id: [u8; 16],
        reason: String,
    },
    Ping {
        timestamp_ms: u64,
    },
    Pong {
        timestamp_ms: u64,
    },
    Bye,
}

// ── mDNS / defaults ──────────────────────────────────────────────────────────

pub const MDNS_SERVICE_TYPE: &str = "_cliprelay._tcp.local.";
pub const PROTOCOL_VERSION: u16 = 3;
pub const DEFAULT_PORT: u16 = 47823;
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Fix 16: ClipboardContent::is_empty ───────────────────────────────────

    #[test]
    fn empty_text_is_empty() {
        assert!(ClipboardContent::Text(String::new()).is_empty());
    }

    #[test]
    fn nonempty_text_is_not_empty() {
        assert!(!ClipboardContent::Text("hello".into()).is_empty());
    }

    #[test]
    fn empty_image_is_empty() {
        let c = ClipboardContent::Image {
            mime: "image/png".into(),
            data: vec![],
        };
        assert!(c.is_empty());
    }

    #[test]
    fn nonempty_image_is_not_empty() {
        let c = ClipboardContent::Image {
            mime: "image/png".into(),
            data: vec![0xFF; 8],
        };
        assert!(!c.is_empty());
    }

    #[test]
    fn empty_file_is_empty() {
        let c = ClipboardContent::File {
            name: "doc.pdf".into(),
            data: vec![],
        };
        assert!(c.is_empty());
    }

    #[test]
    fn nonempty_file_is_not_empty() {
        let c = ClipboardContent::File {
            name: "doc.pdf".into(),
            data: vec![1, 2, 3],
        };
        assert!(!c.is_empty());
    }

    #[test]
    fn is_empty_consistent_with_byte_len() {
        let items: Vec<ClipboardContent> = vec![
            ClipboardContent::Text(String::new()),
            ClipboardContent::Text("x".into()),
            ClipboardContent::Image { mime: "image/png".into(), data: vec![] },
            ClipboardContent::Image { mime: "image/png".into(), data: vec![0] },
        ];
        for item in &items {
            assert_eq!(item.is_empty(), item.byte_len() == 0);
        }
    }
}
