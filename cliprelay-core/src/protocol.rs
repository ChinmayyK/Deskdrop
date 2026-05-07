//! ClipRelay Protocol — wire format definitions
//!
//! All messages are length-prefixed (u32 LE) + bincode-encoded.
//! After the handshake, every frame is AEAD-encrypted with a
//! per-session ChaCha20-Poly1305 key derived via X25519 ECDH + HKDF.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024;   // 4 MB
pub const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024; // 32 MB
pub const MAX_FILE_BYTES: usize = 512 * 1024 * 1024; // 512 MB (chunked)

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClipboardContent {
    Text(String),
    Image { mime: String, data: Vec<u8> },
    /// File payload — delivered as clipboard and also saved to Downloads/ClipRelay.
    File { name: String, data: Vec<u8> },
}

impl ClipboardContent {
    pub fn byte_len(&self) -> usize {
        match self {
            ClipboardContent::Text(s) => s.len(),
            ClipboardContent::Image { data, .. } => data.len(),
            ClipboardContent::File { data, .. } => data.len(),
        }
    }

    pub fn kind_str(&self) -> &'static str {
        match self {
            ClipboardContent::Text(_) => "text",
            ClipboardContent::Image { .. } => "image",
            ClipboardContent::File { .. } => "file",
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
        Self { hash, timestamp, source_device, kind, bytes, pinned }
    }

    /// Human-readable summary shown in timeline.
    /// Uses friendly device name, NOT internal ID.
    pub fn summary(&self) -> String {
        match self.kind.as_str() {
            "text"  => format!("[{}] copied text", self.source_device),
            "image" => format!("[{}] copied image", self.source_device),
            "file"  => format!("[{}] received file", self.source_device),
            _       => format!("[{}] clipboard item", self.source_device),
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
    },
    HistoryMetadata {
        entry: HistoryMetadata,
    },
    ClipboardAck {
        seq: u64,
    },
    /// Announce a file transfer before sending chunks.
    FileTransferAnnounce {
        meta: FileTransferMetadata,
    },
    /// Receiver accepts or rejects the transfer.
    FileTransferAccept {
        transfer_id: [u8; 16],
        accepted: bool,
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
