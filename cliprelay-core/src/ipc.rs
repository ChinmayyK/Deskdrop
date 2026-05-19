//! ClipRelay IPC — daemon ↔ CLI communication layer.
//!
//! The running daemon exposes a local socket so `cliprelay-cli` can:
//!   - Query live status and connected peers
//!   - Push a clipboard payload manually
//!   - List / revoke trusted devices without restarting
//!   - Stream clipboard history in real-time
//!   - Enable / disable syncing without restarting
//!
//! # Transport
//! - Linux / macOS: Unix domain socket at `$XDG_RUNTIME_DIR/cliprelay.sock`
//!   (or `/tmp/cliprelay-<uid>.sock` as fallback)
//! - Windows: named pipe `\\.\pipe\cliprelay`
//!
//! # Protocol
//! Plain JSON request → JSON response (newline-delimited), both directions.
//! No authentication needed — socket is mode 0600, only the owning user.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ── Request / Response types ──────────────────────────────────────────────────

/// Parse a UUID string, giving a clear error if it's malformed.
/// Used by IPC handlers and `spawn_with_engine`.
pub fn parse_uuid(value: &str) -> anyhow::Result<uuid::Uuid> {
    uuid::Uuid::parse_str(value).with_context(|| format!("invalid UUID: {value}"))
}

/// Decode standard or URL-safe base64. Used by IPC push-image / push-file.
pub fn decode_base64(s: &str) -> anyhow::Result<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(s))
        .context("base64 decode failed")
}

/// Parse a transfer-ID string (hex UUID without hyphens, or standard UUID).
pub fn parse_transfer_id(s: &str) -> anyhow::Result<[u8; 16]> {
    // Accept standard UUID form (with dashes) or 32-char hex.
    let uid = if s.len() == 32 {
        let mut bytes = [0u8; 16];
        hex::decode_to_slice(s, &mut bytes).context("invalid transfer id hex")?;
        bytes
    } else {
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(
            uuid::Uuid::parse_str(s)
                .context("invalid transfer id uuid")?
                .as_bytes(),
        );
        bytes
    };
    Ok(uid)
}

/// Flattened partial-settings struct used by `SaveSettings` IPC variant.
pub struct PartialSettings {
    pub port: Option<u16>,
    pub device_name: Option<String>,
    pub sync_enabled: Option<bool>,
    pub sync_text: Option<bool>,
    pub sync_images: Option<bool>,
    pub sync_files: Option<bool>,
    pub history_limit: Option<usize>,
    pub max_history_text_bytes: Option<usize>,
    pub max_payload_bytes: Option<u64>,
    pub clipboard_poll_ms: Option<u64>,
    pub max_pushes_per_sec: Option<f64>,
    pub rate_limit_burst: Option<f64>,
    pub smart_sync_duplicate_window_ms: Option<u64>,
    pub smart_sync_debounce_ms: Option<u64>,
    pub block_sensitive_text: Option<bool>,
    pub require_tofu_confirmation: Option<bool>,
    pub show_receive_notification: Option<bool>,
    pub ignore_patterns: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum IpcRequest {
    /// Get daemon status snapshot.
    Status,
    /// List connected peers with stats.
    Peers,
    /// List trusted devices from the store.
    TrustedDevices,
    /// Connect to a peer manually by IP and port.
    ConnectPeer { ip: String, port: u16 },
    /// Disconnect from a peer explicitly.
    DisconnectPeer { device_id: String },
    /// Mark a peer as trusted.
    TrustPeer { device_id: String },
    /// Reject a peer without trusting it.
    RejectPeer { device_id: String },
    /// Revoke a trusted device by UUID.
    RevokeTrustedDevice { device_id: String },
    /// Push clipboard text to all peers.
    PushText { text: String },
    /// Push clipboard text to one peer.
    PushTextTo { text: String, target: String },
    /// Push clipboard image bytes to all peers.
    PushImage { mime: String, data_base64: String },
    /// Push clipboard file bytes to all peers.
    PushFile { name: String, data_base64: String },
    /// Record local clipboard text in history without syncing.
    RememberText { text: String },
    /// Get recent history entries.
    History { last: usize },
    /// Search history.
    HistorySearch { query: String, limit: usize },
    /// Re-push a history item, optionally to one peer.
    HistoryRepush { id: u64, target: Option<String> },
    /// Pin or unpin a history item.
    HistoryPin { id: u64, pinned: bool },
    /// Delete one history item.
    HistoryDelete { id: u64 },
    /// Clear all history.
    HistoryClear,
    /// Export full history as CSV text.
    HistoryExportCsv,
    /// Read recent feedback events.
    Feedback { last: usize },
    /// Fetch one incoming clipboard payload by daemon-assigned ID.
    IncomingClipboard { id: u64 },
    /// Show details for one trusted device.
    DeviceDetails { device_id: String },
    /// Assign a friendly display name to a trusted device.
    RenameTrustedDevice {
        device_id: String,
        display_name: String,
    },
    /// Pause clipboard sync for a device (keeps connection alive).
    PauseSyncPeer { device_id: String },
    /// Resume clipboard sync for a device.
    ResumeSyncPeer { device_id: String },
    /// Forget a device pairing (removes from remembered peers, keeps trust).
    ForgetDevice { device_id: String },
    /// Set auto-connect for a device.
    SetAutoConnect { device_id: String, enabled: bool },
    /// Get recent activity feed entries.
    ActivityRecent { limit: usize },
    /// Get activity feed entries since a given entry ID.
    ActivitySince { since_id: u64 },
    /// Get pending remote clipboard items not yet applied locally.
    PendingRemoteClipboards,
    /// Explicitly apply a remote clipboard item by its content hash.
    ApplyClipboard { content_hash: String },
    /// Send a file to a specific peer or all peers.
    SendFile {
        name: String,
        mime: String,
        data_base64: String,
        target_device: Option<String>,
    },
    SendFilePath {
        path: String,
        name: String,
        mime: String,
        target_device: Option<String>,
    },
    /// Accept an incoming file transfer.
    AcceptFileTransfer { transfer_id: String },
    /// Reject an incoming file transfer.
    RejectFileTransfer { transfer_id: String, reason: String },
    /// Cancel an active file transfer.
    CancelFileTransfer { transfer_id: String },
    /// Update timeline-first clipboard mode.
    SetTimelineFirstMode { enabled: bool },
    /// Update auto-apply remote clipboard setting.
    SetAutoApplyClipboard { enabled: bool },
    /// Get current settings.
    GetSettings,
    /// Apply a partial JSON patch to settings.
    PatchSettings { patch: String },
    /// Enable / disable sync.
    SetSyncEnabled { enabled: bool },
    /// Gracefully stop the daemon.
    Shutdown,
    /// Ping — check if daemon is alive.
    Ping,
    /// Get a serializable snapshot of global runtime metrics.
    GetMetrics,

    // ── History tag management ────────────────────────────────────────────────
    /// Add a tag to a history entry.
    HistoryTag { id: u64, tag: String },
    /// Remove a tag from a history entry.
    HistoryUntag { id: u64, tag: String },

    // ── History stats & export ────────────────────────────────────────────────
    /// Return aggregated statistics over the history buffer.
    HistoryStats,
    /// Export full history as a JSON array.
    HistoryExportJson,

    // ── Filtered history ──────────────────────────────────────────────────────
    /// Return a filtered list of history entries.
    HistoryFilteredList {
        /// Restrict to "text", "image", "file", or "metadata".
        kind: Option<String>,
        /// Case-insensitive device name substring.
        device: Option<String>,
        /// Include only entries at or after this Unix timestamp.
        from_secs: Option<u64>,
        /// Include only entries at or before this Unix timestamp.
        to_secs: Option<u64>,
        /// Must contain this tag.
        tag: Option<String>,
        /// Maximum results to return.
        limit: usize,
        /// If true, only return pinned entries.
        pinned_only: bool,
    },

    // ── Clipboard templates ───────────────────────────────────────────────────
    /// List all configured clipboard templates.
    TemplateList,
    /// Push a named template to all (or one) peer.
    TemplatePush {
        name: String,
        target_device: Option<String>,
    },
    /// Add or replace a clipboard template.
    TemplateSet {
        name: String,
        text: String,
        description: String,
    },
    /// Remove a clipboard template by name.
    TemplateRemove { name: String },

    // ── Per-peer settings ─────────────────────────────────────────────────────
    /// Get per-peer settings for a specific device.
    GetPeerSettings { device_id: String },
    /// Patch per-peer settings for a specific device (partial JSON).
    PatchPeerSettings { device_id: String, patch: String },
    /// Re-trigger mDNS/NSD discovery — called by Mac "Scan" button.
    RescanPeers,
    /// Connect to a peer by hostname:port (with DNS resolution).
    /// Used by Mac manual-connect field; `port` defaults to the daemon's
    /// configured port if omitted.
    ConnectManual {
        host: String,
        #[serde(default)]
        port: Option<u16>,
    },
    /// Re-push a previously-received clipboard item identified by its content hash.
    /// Optionally restrict to one target device.
    PushClipboardHash {
        hash: String,
        #[serde(default)]
        target_device_id: Option<String>,
    },
    /// Push the current system clipboard to all (or one) peer without providing the text inline.
    /// The daemon reads the clipboard itself via the platform clipboard API.
    PushClipboard {
        #[serde(default)]
        target_device_id: Option<String>,
    },
    /// Persist a full settings snapshot from the Mac preferences UI.
    SaveSettings {
        #[serde(default)]
        port: Option<u16>,
        #[serde(default)]
        device_name: Option<String>,
        #[serde(default)]
        sync_enabled: Option<bool>,
        #[serde(default)]
        sync_text: Option<bool>,
        #[serde(default)]
        sync_images: Option<bool>,
        #[serde(default)]
        sync_files: Option<bool>,
        #[serde(default)]
        history_limit: Option<usize>,
        #[serde(default)]
        max_history_text_bytes: Option<usize>,
        #[serde(default)]
        max_payload_bytes: Option<u64>,
        #[serde(default)]
        clipboard_poll_ms: Option<u64>,
        #[serde(default)]
        max_pushes_per_sec: Option<f64>,
        #[serde(default)]
        rate_limit_burst: Option<f64>,
        #[serde(default)]
        smart_sync_duplicate_window_ms: Option<u64>,
        #[serde(default)]
        smart_sync_debounce_ms: Option<u64>,
        #[serde(default)]
        block_sensitive_text: Option<bool>,
        #[serde(default)]
        require_tofu_confirmation: Option<bool>,
        #[serde(default)]
        show_receive_notification: Option<bool>,
        #[serde(default)]
        ignore_patterns: Option<Vec<String>>,
    },

    // ── Call continuity ───────────────────────────────────────────────────────
    /// Send a call action (accept/decline) to a ringing Android peer.
    /// Used by the macOS incoming-call banner.
    CallAction {
        /// "accept" or "decline"
        action: String,
        /// Target Android device UUID
        target_device: String,
    },
    /// Push a phone call state change from this Android device to all peers.
    /// Called by the Android service when the phone call state changes.
    PushCallState {
        /// "ringing", "offhook", or "idle"
        state: String,
        /// Raw phone number (may be empty on API 31+)
        number: String,
        /// Resolved contact name (may be empty)
        contact_name: String,
    },
    // ── Battery synchronization (F20) ─────────────────────────────────────────
    /// Push battery status from the local device to all peers.
    PushBatteryStatus {
        /// Battery level (0–100)
        level: u8,
        /// Whether the device is charging
        charging: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum IpcResponse {
    Ok {
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
    },
    Error {
        message: String,
    },
    Pong {
        uptime_secs: u64,
    },
}

impl IpcResponse {
    pub fn ok(data: impl Serialize) -> Self {
        IpcResponse::Ok {
            data: serde_json::to_value(data).ok(),
        }
    }
    pub fn ok_empty() -> Self {
        IpcResponse::Ok { data: None }
    }
    pub fn error(msg: impl Into<String>) -> Self {
        IpcResponse::Error {
            message: msg.into(),
        }
    }

    /// Alias for [`IpcResponse::error`].  Used by the embedded
    /// `spawn_with_engine` handler for brevity.
    #[inline]
    pub fn err(msg: impl Into<String>) -> Self {
        Self::error(msg)
    }
}

// ── Socket path ───────────────────────────────────────────────────────────────

pub fn socket_path() -> PathBuf {
    #[cfg(windows)]
    return PathBuf::from(r"\\.\pipe\cliprelay");

    #[cfg(not(windows))]
    {
        if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
            PathBuf::from(runtime).join("cliprelay.sock")
        } else {
            let uid = unsafe { libc::getuid() };
            PathBuf::from(format!("/tmp/cliprelay-{}.sock", uid))
        }
    }
}

// ── Server (runs inside the daemon) ──────────────────────────────────────────

#[cfg(unix)]
pub mod server {
    use super::*;
    use anyhow::Context;
    use std::sync::Arc;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::{UnixListener, UnixStream};
    use tracing::{debug, info, warn};

    /// Spawn the IPC server. The `handler` closure is called for each request.
    pub async fn spawn<H, Fut>(handler: Arc<H>) -> Result<()>
    where
        H: Fn(IpcRequest) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = IpcResponse> + Send + 'static,
    {
        let path = socket_path();

        // Remove stale socket from previous run.
        if path.exists() {
            std::fs::remove_file(&path).ok();
        }

        let listener = UnixListener::bind(&path)
            .with_context(|| format!("binding IPC socket at {:?}", path))?;

        // Restrict to owner only.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }

        info!("IPC server listening at {:?}", path);

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let handler = handler.clone();
                        tokio::spawn(handle_connection(stream, handler));
                    }
                    Err(e) => warn!("IPC accept error: {}", e),
                }
            }
        });

        Ok(())
    }

    pub async fn spawn_with_engine(engine: std::sync::Arc<crate::engine::Engine>) -> Result<()> {
        super::client::spawn_with_engine(engine).await
    }

    async fn handle_connection<H, Fut>(stream: UnixStream, handler: Arc<H>)
    where
        H: Fn(IpcRequest) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = IpcResponse> + Send + 'static,
    {
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();

        while let Ok(Some(line)) = lines.next_line().await {
            let response = match serde_json::from_str::<IpcRequest>(&line) {
                Ok(req) => {
                    // LOW-07: never log the raw IPC request — it may contain
                    // clipboard text, passwords, or other private content.
                    // Log only the command discriminant (the tag field).
                    let cmd_tag = serde_json::to_value(&req)
                        .ok()
                        .and_then(|v| v.get("cmd").and_then(|c| c.as_str()).map(String::from))
                        .unwrap_or_else(|| "<unknown>".into());
                    debug!(cmd = %cmd_tag, "IPC request received");
                    handler(req).await
                }
                Err(e) => IpcResponse::error(format!("parse error: {}", e)),
            };

            let mut resp_bytes = serde_json::to_vec(&response).unwrap_or_default();
            resp_bytes.push(b'\n');
            if writer.write_all(&resp_bytes).await.is_err() {
                break;
            }
        }
    }
}

// ── Client (used by cliprelay-cli) ──────────────────────────────────────────

#[cfg(unix)]
pub mod client {
    use super::*;
    use anyhow::Context;
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    pub struct IpcClient {
        stream: UnixStream,
    }

    impl IpcClient {
        /// Connect to the running daemon. Fails fast if daemon is not running.
        pub async fn connect() -> Result<Self> {
            let path = socket_path();
            let stream =
                tokio::time::timeout(Duration::from_millis(500), UnixStream::connect(&path))
                    .await
                    .with_context(|| "daemon not responding")?
                    .with_context(|| {
                        format!("connecting to {:?} — is the daemon running?", path)
                    })?;
            Ok(Self { stream })
        }

        /// Send a request and receive one response.
        /// A 10-second timeout guards against a hung daemon causing the CLI
        /// to block indefinitely (HIGH-04).
        pub async fn request(&mut self, req: &IpcRequest) -> Result<IpcResponse> {
            let (reader, mut writer) = self.stream.split();
            let mut reader = BufReader::new(reader);

            let mut req_bytes = serde_json::to_vec(req)?;
            req_bytes.push(b'\n');
            writer.write_all(&req_bytes).await.context("IPC write")?;

            let mut line = String::new();
            tokio::time::timeout(Duration::from_secs(10), reader.read_line(&mut line))
                .await
                .with_context(|| "daemon did not respond within 10 s")?
                .context("IPC read")?;
            serde_json::from_str(&line).context("IPC response parse")
        }
    }

    /// Convenience: spawn the IPC server wired directly to an `Arc<Engine>`.
    ///
    /// This is used by the Linux binary (which embeds the engine) so it doesn't
    /// need to duplicate the full daemon dispatch table.  The handler maps every
    /// `IpcRequest` variant to the corresponding `engine.*` call.
    pub async fn spawn_with_engine(engine: std::sync::Arc<crate::engine::Engine>) -> Result<()> {
        let handler = std::sync::Arc::new(move |req: IpcRequest| {
            let eng = engine.clone();
            async move {
                use crate::engine::Engine;
                match req {
                    IpcRequest::Status => {
                        let snap = eng.status_snapshot().await;
                        let fp = eng.local_fingerprint();
                        let pending = eng.pending_remote_clipboards().await.len();
                        let active_call = eng.active_call().await;
                        let peer_batteries = eng.peer_batteries().await;
                        IpcResponse::ok(serde_json::json!({
                            "peers": snap.peers,
                            "peer_count": snap.peers.iter().filter(|p| p.status == crate::peer_manager::PeerConnectionState::Connected).count(),
                            "last_sync_at": snap.last_sync_at,
                            "pending_clipboard_count": pending,
                            "local_fingerprint": fp,
                            "active_call": active_call,
                            "peer_batteries": peer_batteries,
                        }))
                    }
                    IpcRequest::RescanPeers => {
                        eng.rescan_peers().await;
                        IpcResponse::ok_empty()
                    }
                    IpcRequest::Peers => IpcResponse::ok(eng.status_snapshot().await.peers),
                    IpcRequest::TrustedDevices => IpcResponse::ok(eng.trusted_devices().await),
                    IpcRequest::ActivityRecent { limit } => {
                        IpcResponse::ok(eng.activity_recent(limit).await)
                    }
                    IpcRequest::ActivitySince { since_id } => {
                        IpcResponse::ok(eng.activity_since(since_id).await)
                    }
                    IpcRequest::PendingRemoteClipboards => {
                        IpcResponse::ok(eng.pending_remote_clipboards().await)
                    }
                    IpcRequest::ApplyClipboard { content_hash } => {
                        match eng.apply_clipboard_by_hash(content_hash).await {
                            Ok(_) => IpcResponse::ok_empty(),
                            Err(e) => IpcResponse::err(e.to_string()),
                        }
                    }
                    IpcRequest::ConnectPeer { ip, port } => {
                        match eng.connect_to_peer(ip, port).await {
                            Ok(()) => IpcResponse::ok_empty(),
                            Err(e) => IpcResponse::err(e.to_string()),
                        }
                    }
                    IpcRequest::TrustPeer { device_id } => {
                        match crate::ipc::parse_uuid(&device_id)
                            .and_then(|id| Ok(id))
                            .ok()
                            .map(|id| eng.trust_peer(id))
                        {
                            Some(fut) => match fut.await {
                                Ok(_) => IpcResponse::ok_empty(),
                                Err(e) => IpcResponse::err(e.to_string()),
                            },
                            None => IpcResponse::err("invalid device id"),
                        }
                    }
                    IpcRequest::RejectPeer { device_id } => {
                        match crate::ipc::parse_uuid(&device_id)
                            .ok()
                            .map(|id| eng.reject_peer(id))
                        {
                            Some(fut) => match fut.await {
                                Ok(_) => IpcResponse::ok_empty(),
                                Err(e) => IpcResponse::err(e.to_string()),
                            },
                            None => IpcResponse::err("invalid device id"),
                        }
                    }
                    IpcRequest::Ping => IpcResponse::ok_empty(),
                    IpcRequest::Shutdown => {
                        std::process::exit(0);
                    }
                    // ── Call continuity ────────────────────────────────────────────────
                    IpcRequest::CallAction { action, target_device } => {
                        match crate::ipc::parse_uuid(&target_device) {
                            Ok(uuid) => {
                                eng.send_call_action(action, uuid).await;
                                IpcResponse::ok_empty()
                            }
                            Err(e) => IpcResponse::error(format!("bad target_device: {e}")),
                        }
                    }
                    IpcRequest::PushCallState { state, number, contact_name } => {
                        eng.push_call_state(state, number, contact_name).await;
                        IpcResponse::ok_empty()
                    }
                    IpcRequest::PushBatteryStatus { level, charging } => {
                        eng.push_battery_status(level, charging).await;
                        IpcResponse::ok_empty()
                    }
                    // ── Metrics ────────────────────────────────────────────────────────
                    IpcRequest::GetMetrics => {
                        let snap = eng.status_snapshot().await;
                        IpcResponse::ok(serde_json::json!({
                            "connected_peers": snap.peers.len(),
                            "sync_eligible": snap.peers.iter().filter(|p| p.is_sync_eligible()).count(),
                            "device_id": eng.device_id().await,
                        }))
                    }
                    // ── History ────────────────────────────────────────────────────────
                    IpcRequest::History { last } => IpcResponse::ok(eng.history_recent(last).await),
                    IpcRequest::HistorySearch { query, limit } => {
                        IpcResponse::ok(eng.history_search(query, limit).await)
                    }
                    IpcRequest::HistoryRepush { id, target } => {
                        let tgt = match target {
                            Some(ref s) => match crate::ipc::parse_uuid(s) {
                                Ok(uid) => crate::engine::SyncTarget::Device(uid),
                                Err(_) => return IpcResponse::err("invalid target device id"),
                            },
                            None => crate::engine::SyncTarget::All,
                        };
                        match eng.history_repush(id, tgt).await {
                            Ok(_) => IpcResponse::ok_empty(),
                            Err(e) => IpcResponse::err(e.to_string()),
                        }
                    }
                    IpcRequest::HistoryPin { id, pinned } => {
                        match eng.history_set_pinned(id, pinned).await {
                            Ok(_) => IpcResponse::ok_empty(),
                            Err(e) => IpcResponse::err(e.to_string()),
                        }
                    }
                    IpcRequest::HistoryDelete { id } => match eng.history_delete(id).await {
                        Ok(removed) => {
                            if removed {
                                IpcResponse::ok_empty()
                            } else {
                                IpcResponse::err("entry not found")
                            }
                        }
                        Err(e) => IpcResponse::err(e.to_string()),
                    },
                    IpcRequest::HistoryClear => match eng.history_clear().await {
                        Ok(_) => IpcResponse::ok_empty(),
                        Err(e) => IpcResponse::err(e.to_string()),
                    },
                    IpcRequest::HistoryExportCsv => IpcResponse::ok(eng.history_export_csv().await),
                    IpcRequest::HistoryExportJson => match eng.history_export_json().await {
                        Ok(json) => IpcResponse::ok(json),
                        Err(e) => IpcResponse::err(e.to_string()),
                    },
                    IpcRequest::HistoryStats => IpcResponse::ok(eng.history_stats().await),
                    IpcRequest::HistoryTag { id, tag } => {
                        match eng.history_add_tag(id, tag).await {
                            Ok(_) => IpcResponse::ok_empty(),
                            Err(e) => IpcResponse::err(e.to_string()),
                        }
                    }
                    IpcRequest::HistoryUntag { id, tag } => {
                        match eng.history_remove_tag(id, tag).await {
                            Ok(_) => IpcResponse::ok_empty(),
                            Err(e) => IpcResponse::err(e.to_string()),
                        }
                    }
                    IpcRequest::HistoryFilteredList {
                        kind,
                        device,
                        from_secs,
                        to_secs,
                        tag,
                        limit,
                        pinned_only,
                    } => IpcResponse::ok(
                        eng.history_filtered(
                            kind,
                            device,
                            from_secs,
                            to_secs,
                            tag,
                            limit,
                            pinned_only,
                        )
                        .await,
                    ),
                    IpcRequest::RememberText { text } => match eng.remember_text(text).await {
                        Ok(_) => IpcResponse::ok_empty(),
                        Err(e) => IpcResponse::err(e.to_string()),
                    },
                    IpcRequest::IncomingClipboard { id } => {
                        IpcResponse::ok(eng.incoming_clipboard(id).await)
                    }
                    // ── Peer management ────────────────────────────────────────────────
                    IpcRequest::DisconnectPeer { device_id } => {
                        match crate::ipc::parse_uuid(&device_id)
                            .ok()
                            .map(|id| eng.disconnect_peer(id))
                        {
                            Some(fut) => match fut.await {
                                Ok(true) => IpcResponse::ok_empty(),
                                Ok(false) => IpcResponse::err("peer not connected"),
                                Err(e) => IpcResponse::err(e.to_string()),
                            },
                            None => IpcResponse::err("invalid device id"),
                        }
                    }
                    IpcRequest::RevokeTrustedDevice { device_id } => {
                        match crate::ipc::parse_uuid(&device_id)
                            .ok()
                            .map(|id| eng.revoke_peer(id))
                        {
                            Some(fut) => match fut.await {
                                Ok(true) => IpcResponse::ok_empty(),
                                Ok(false) => IpcResponse::err("device not found"),
                                Err(e) => IpcResponse::err(e.to_string()),
                            },
                            None => IpcResponse::err("invalid device id"),
                        }
                    }
                    IpcRequest::PauseSyncPeer { device_id } => {
                        match crate::ipc::parse_uuid(&device_id)
                            .ok()
                            .map(|id| eng.pause_sync_peer(id))
                        {
                            Some(fut) => match fut.await {
                                Ok(_) => IpcResponse::ok_empty(),
                                Err(e) => IpcResponse::err(e.to_string()),
                            },
                            None => IpcResponse::err("invalid device id"),
                        }
                    }
                    IpcRequest::ResumeSyncPeer { device_id } => {
                        match crate::ipc::parse_uuid(&device_id)
                            .ok()
                            .map(|id| eng.resume_sync_peer(id))
                        {
                            Some(fut) => match fut.await {
                                Ok(_) => IpcResponse::ok_empty(),
                                Err(e) => IpcResponse::err(e.to_string()),
                            },
                            None => IpcResponse::err("invalid device id"),
                        }
                    }
                    IpcRequest::ForgetDevice { device_id } => {
                        match crate::ipc::parse_uuid(&device_id)
                            .ok()
                            .map(|id| eng.forget_device(id))
                        {
                            Some(fut) => match fut.await {
                                Ok(true) => IpcResponse::ok_empty(),
                                Ok(false) => IpcResponse::err("device not found"),
                                Err(e) => IpcResponse::err(e.to_string()),
                            },
                            None => IpcResponse::err("invalid device id"),
                        }
                    }
                    IpcRequest::SetAutoConnect { device_id, enabled } => {
                        match crate::ipc::parse_uuid(&device_id)
                            .ok()
                            .map(|id| eng.set_auto_connect(id, enabled))
                        {
                            Some(fut) => match fut.await {
                                Ok(_) => IpcResponse::ok_empty(),
                                Err(e) => IpcResponse::err(e.to_string()),
                            },
                            None => IpcResponse::err("invalid device id"),
                        }
                    }
                    IpcRequest::RenameTrustedDevice {
                        device_id,
                        display_name,
                    } => {
                        match crate::ipc::parse_uuid(&device_id)
                            .ok()
                            .map(|id| eng.rename_trusted_device(id, display_name.clone()))
                        {
                            Some(fut) => match fut.await {
                                Ok(_) => IpcResponse::ok_empty(),
                                Err(e) => IpcResponse::err(e.to_string()),
                            },
                            None => IpcResponse::err("invalid device id"),
                        }
                    }
                    IpcRequest::DeviceDetails { device_id } => {
                        match crate::ipc::parse_uuid(&device_id) {
                            Ok(id) => IpcResponse::ok(eng.device_details(id).await),
                            Err(_) => IpcResponse::err("invalid device id"),
                        }
                    }
                    IpcRequest::ConnectManual { host, port } => {
                        let p = port.unwrap_or(eng.current_settings().await.port);
                        match eng.connect_to_peer(host, p).await {
                            Ok(()) => IpcResponse::ok_empty(),
                            Err(e) => IpcResponse::err(e.to_string()),
                        }
                    }
                    IpcRequest::GetPeerSettings { device_id } => {
                        match crate::ipc::parse_uuid(&device_id) {
                            Ok(id) => IpcResponse::ok(eng.get_peer_settings(id).await),
                            Err(_) => IpcResponse::err("invalid device id"),
                        }
                    }
                    IpcRequest::PatchPeerSettings { device_id, patch } => {
                        match crate::ipc::parse_uuid(&device_id) {
                            Ok(id) => match eng.patch_peer_settings(id, patch).await {
                                Ok(_) => IpcResponse::ok_empty(),
                                Err(e) => IpcResponse::err(e.to_string()),
                            },
                            Err(_) => IpcResponse::err("invalid device id"),
                        }
                    }
                    // ── Push operations ────────────────────────────────────────────────
                    IpcRequest::PushText { text } => {
                        let content = crate::protocol::ClipboardContent::Text(text);
                        let n = eng.push_clipboard(content).await;
                        IpcResponse::ok(serde_json::json!({ "delivered": n }))
                    }
                    IpcRequest::PushTextTo { text, target } => {
                        match crate::ipc::parse_uuid(&target) {
                            Ok(id) => {
                                let content = crate::protocol::ClipboardContent::Text(text);
                                eng.push_clipboard_to(
                                    content,
                                    crate::engine::SyncTarget::Device(id),
                                )
                                .await;
                                IpcResponse::ok_empty()
                            }
                            Err(_) => IpcResponse::err("invalid target device id"),
                        }
                    }
                    IpcRequest::PushImage { mime, data_base64 } => {
                        match crate::ipc::decode_base64(&data_base64) {
                            Ok(data) => {
                                let content =
                                    crate::protocol::ClipboardContent::Image { mime, data };
                                let n = eng.push_clipboard(content).await;
                                IpcResponse::ok(serde_json::json!({ "delivered": n }))
                            }
                            Err(e) => IpcResponse::err(format!("base64 decode: {}", e)),
                        }
                    }
                    IpcRequest::PushFile { name, data_base64 } => {
                        match crate::ipc::decode_base64(&data_base64) {
                            Ok(data) => {
                                let content =
                                    crate::protocol::ClipboardContent::File { name, data };
                                let n = eng.push_clipboard(content).await;
                                IpcResponse::ok(serde_json::json!({ "delivered": n }))
                            }
                            Err(e) => IpcResponse::err(format!("base64 decode: {}", e)),
                        }
                    }
                    IpcRequest::PushClipboardHash {
                        hash,
                        target_device_id,
                    } => {
                        let tgt = match target_device_id {
                            Some(ref s) => match crate::ipc::parse_uuid(s) {
                                Ok(id) => crate::engine::SyncTarget::Device(id),
                                Err(_) => return IpcResponse::err("invalid target device id"),
                            },
                            None => crate::engine::SyncTarget::All,
                        };
                        match eng.repush_clipboard_hash(hash, tgt).await {
                            Ok(_) => IpcResponse::ok_empty(),
                            Err(e) => IpcResponse::err(e.to_string()),
                        }
                    }
                    IpcRequest::PushClipboard { target_device_id } => {
                        let tgt = match target_device_id {
                            Some(ref s) => match crate::ipc::parse_uuid(s) {
                                Ok(id) => crate::engine::SyncTarget::Device(id),
                                Err(_) => return IpcResponse::err("invalid target device id"),
                            },
                            None => crate::engine::SyncTarget::All,
                        };
                        match eng.push_current_clipboard(tgt).await {
                            Ok(_) => IpcResponse::ok_empty(),
                            Err(e) => IpcResponse::err(e.to_string()),
                        }
                    }
                    // ── File transfers ─────────────────────────────────────────────────
                    IpcRequest::SendFile {
                        name,
                        mime,
                        data_base64,
                        target_device,
                    } => {
                        let tgt = match target_device {
                            Some(ref s) => match crate::ipc::parse_uuid(s) {
                                Ok(id) => Some(id),
                                Err(_) => return IpcResponse::err("invalid target device id"),
                            },
                            None => None,
                        };
                        match crate::ipc::decode_base64(&data_base64) {
                            Ok(data) => match eng.send_file(data, name, mime, tgt).await {
                                Ok(_) => IpcResponse::ok_empty(),
                                Err(e) => IpcResponse::err(e.to_string()),
                            },
                            Err(e) => IpcResponse::err(format!("base64 decode: {}", e)),
                        }
                    }
                    IpcRequest::SendFilePath {
                        path,
                        name,
                        mime,
                        target_device,
                    } => {
                        let tgt = match target_device {
                            Some(ref s) => match crate::ipc::parse_uuid(s) {
                                Ok(id) => Some(id),
                                Err(_) => return IpcResponse::err("invalid target device id"),
                            },
                            None => None,
                        };
                        match eng
                            .send_file_path(std::path::PathBuf::from(path), name, mime, tgt)
                            .await
                        {
                            Ok(transfer_id) => IpcResponse::ok(hex::encode(transfer_id)),
                            Err(e) => IpcResponse::err(e.to_string()),
                        }
                    }
                    IpcRequest::AcceptFileTransfer { transfer_id } => {
                        match crate::ipc::parse_transfer_id(&transfer_id) {
                            Ok(id) => match eng.accept_file_transfer(id).await {
                                Ok(_) => IpcResponse::ok_empty(),
                                Err(e) => IpcResponse::err(e.to_string()),
                            },
                            Err(_) => IpcResponse::err("invalid transfer id"),
                        }
                    }
                    IpcRequest::RejectFileTransfer {
                        transfer_id,
                        reason,
                    } => match crate::ipc::parse_transfer_id(&transfer_id) {
                        Ok(id) => match eng.reject_file_transfer(id, reason).await {
                            Ok(_) => IpcResponse::ok_empty(),
                            Err(e) => IpcResponse::err(e.to_string()),
                        },
                        Err(_) => IpcResponse::err("invalid transfer id"),
                    },
                    IpcRequest::CancelFileTransfer { transfer_id } => {
                        match crate::ipc::parse_transfer_id(&transfer_id) {
                            Ok(id) => match eng.cancel_file_transfer(id).await {
                                Ok(_) => IpcResponse::ok_empty(),
                                Err(e) => IpcResponse::err(e.to_string()),
                            },
                            Err(_) => IpcResponse::err("invalid transfer id"),
                        }
                    }
                    // ── Settings ───────────────────────────────────────────────────────
                    IpcRequest::GetSettings => IpcResponse::ok(eng.current_settings().await),
                    IpcRequest::PatchSettings { patch } => match eng.patch_settings(patch).await {
                        Ok(_) => IpcResponse::ok_empty(),
                        Err(e) => IpcResponse::err(e.to_string()),
                    },
                    IpcRequest::SetSyncEnabled { enabled } => {
                        eng.set_sync_enabled(enabled).await;
                        IpcResponse::ok_empty()
                    }
                    IpcRequest::SetTimelineFirstMode { enabled } => {
                        eng.set_timeline_first_mode(enabled).await;
                        IpcResponse::ok_empty()
                    }
                    IpcRequest::SetAutoApplyClipboard { enabled } => {
                        eng.set_auto_apply_clipboard(enabled).await;
                        IpcResponse::ok_empty()
                    }
                    IpcRequest::SaveSettings {
                        port,
                        device_name,
                        sync_enabled,
                        sync_text,
                        sync_images,
                        sync_files,
                        history_limit,
                        max_history_text_bytes,
                        max_payload_bytes,
                        clipboard_poll_ms,
                        max_pushes_per_sec,
                        rate_limit_burst,
                        smart_sync_duplicate_window_ms,
                        smart_sync_debounce_ms,
                        block_sensitive_text,
                        require_tofu_confirmation,
                        show_receive_notification,
                        ignore_patterns,
                    } => {
                        match eng
                            .save_settings_partial(crate::ipc::PartialSettings {
                                port,
                                device_name,
                                sync_enabled,
                                sync_text,
                                sync_images,
                                sync_files,
                                history_limit,
                                max_history_text_bytes,
                                max_payload_bytes,
                                clipboard_poll_ms,
                                max_pushes_per_sec,
                                rate_limit_burst,
                                smart_sync_duplicate_window_ms,
                                smart_sync_debounce_ms,
                                block_sensitive_text,
                                require_tofu_confirmation,
                                show_receive_notification,
                                ignore_patterns,
                            })
                            .await
                        {
                            Ok(_) => IpcResponse::ok_empty(),
                            Err(e) => IpcResponse::err(e.to_string()),
                        }
                    }
                    // ── Templates ──────────────────────────────────────────────────────
                    IpcRequest::TemplateList => IpcResponse::ok(eng.template_list().await),
                    IpcRequest::TemplatePush {
                        name,
                        target_device,
                    } => {
                        let tgt = match target_device {
                            Some(ref s) => match crate::ipc::parse_uuid(s) {
                                Ok(id) => crate::engine::SyncTarget::Device(id),
                                Err(_) => return IpcResponse::err("invalid target device id"),
                            },
                            None => crate::engine::SyncTarget::All,
                        };
                        match eng.template_push(name, tgt).await {
                            Ok(_) => IpcResponse::ok_empty(),
                            Err(e) => IpcResponse::err(e.to_string()),
                        }
                    }
                    IpcRequest::TemplateSet {
                        name,
                        text,
                        description,
                    } => match eng.template_set(name, text, description).await {
                        Ok(_) => IpcResponse::ok_empty(),
                        Err(e) => IpcResponse::err(e.to_string()),
                    },
                    IpcRequest::TemplateRemove { name } => match eng.template_remove(name).await {
                        Ok(found) => {
                            if found {
                                IpcResponse::ok_empty()
                            } else {
                                IpcResponse::err("template not found")
                            }
                        }
                        Err(e) => IpcResponse::err(e.to_string()),
                    },
                    // ── Feedback ───────────────────────────────────────────────────────
                    IpcRequest::Feedback { last } => {
                        IpcResponse::ok(eng.feedback_recent(last).await)
                    }
                    _ => IpcResponse::err("not supported in this build"),
                }
            }
        });
        super::server::spawn(handler).await
    }
}

// ── Windows named pipe stubs ──────────────────────────────────────────────────

#[cfg(windows)]
pub mod server {
    use super::*;
    pub async fn spawn<H, Fut>(_handler: std::sync::Arc<H>) -> Result<()>
    where
        H: Fn(IpcRequest) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = IpcResponse> + Send + 'static,
    {
        // Named pipe implementation uses tokio::net::windows::named_pipe.
        // Stubbed here — full implementation follows the same pattern as Unix.
        tracing::info!("IPC server (Windows named pipe) not yet implemented in this build.");
        Ok(())
    }
}

#[cfg(windows)]
pub mod client {
    use super::*;
    pub struct IpcClient;
    impl IpcClient {
        pub async fn connect() -> Result<Self> {
            anyhow::bail!("Windows IPC client not yet implemented");
        }
        pub async fn request(&mut self, _req: &IpcRequest) -> Result<IpcResponse> {
            anyhow::bail!("Windows IPC client not yet implemented");
        }
    }
}
