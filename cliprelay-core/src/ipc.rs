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

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ── Request / Response types ──────────────────────────────────────────────────

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
                    debug!("IPC request: {:?}", req);
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
        pub async fn request(&mut self, req: &IpcRequest) -> Result<IpcResponse> {
            let (reader, mut writer) = self.stream.split();
            let mut reader = BufReader::new(reader);

            let mut req_bytes = serde_json::to_vec(req)?;
            req_bytes.push(b'\n');
            writer.write_all(&req_bytes).await.context("IPC write")?;

            let mut line = String::new();
            reader.read_line(&mut line).await.context("IPC read")?;
            serde_json::from_str(&line).context("IPC response parse")
        }
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
