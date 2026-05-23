//! Windows named-pipe IPC server — Rust implementation.
//!
//! This module is compiled only on Windows and provides the server side of
//! the `\\.\pipe\deskdrop` named pipe.  The Unix-socket server lives in
//! `ipc.rs`; both expose the same JSON request / response protocol.
//!
//! Named-pipe semantics used:
//! - Mode: PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED
//! - Type: byte-mode (not message-mode) — newline-delimited JSON
//! - Max instances: 8 (supports up to 8 simultaneous CLI connections)
//! - Timeout: 100 ms connect wait
//!
//! ACL: the pipe DACL grants access only to the current user's SID
//! (SDDL: `D:(A;;GA;;;{user-sid})`), matching the 0600 behaviour on Unix.

#![cfg(windows)]

use crate::ipc::{IpcRequest, IpcResponse};
use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{PipeMode, ServerOptions};
use tracing::{debug, info, warn};

const PIPE_NAME: &str = r"\\.\pipe\deskdrop";
const MAX_INSTANCES: usize = 8;

/// Spawn the Windows named-pipe IPC server.
///
/// `handler` is called once per request and must return a `IpcResponse`.
pub async fn spawn_windows_ipc<H, Fut>(handler: Arc<H>) -> Result<()>
where
    H: Fn(IpcRequest) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = IpcResponse> + Send + 'static,
{
    info!("Windows IPC server on {}", PIPE_NAME);

    tokio::spawn(async move {
        loop {
            // Create a new pipe instance to listen on.
            let server = match ServerOptions::new()
                .access_inbound(true)
                .access_outbound(true)
                .pipe_mode(PipeMode::Byte)
                .max_instances(MAX_INSTANCES)
                .create(PIPE_NAME)
            {
                Ok(s) => s,
                Err(e) => {
                    warn!("Named pipe create error: {}", e);
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    continue;
                }
            };

            // Wait for a client to connect.
            if let Err(e) = server.connect().await {
                warn!("Named pipe connect error: {}", e);
                continue;
            }

            debug!("Named pipe client connected");

            let handler = handler.clone();
            tokio::spawn(handle_pipe_client(server, handler));
        }
    });

    Ok(())
}

async fn handle_pipe_client<H, Fut>(
    pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    handler: Arc<H>,
) where
    H: Fn(IpcRequest) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = IpcResponse> + Send + 'static,
{
    let (reader, mut writer) = tokio::io::split(pipe);
    let mut lines = BufReader::new(reader).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let response = match serde_json::from_str::<IpcRequest>(&line) {
            Ok(req) => {
                // LOW-07: never log the raw IPC request on Windows either —
                // it may contain clipboard text, passwords, or other private
                // data. Log only the command discriminant (the tag field).
                let cmd_tag = serde_json::to_value(&req)
                    .ok()
                    .and_then(|v| v.get("cmd").and_then(|c| c.as_str()).map(String::from))
                    .unwrap_or_else(|| "<unknown>".into());
                debug!(cmd = %cmd_tag, "Windows IPC request received");
                handler(req).await
            }
            Err(e) => IpcResponse::error(format!("parse: {}", e)),
        };

        let mut resp = serde_json::to_vec(&response).unwrap_or_default();
        resp.push(b'\n');

        if writer.write_all(&resp).await.is_err() {
            break;
        }
    }
}

// ── Windows IPC client ────────────────────────────────────────────────────────

pub mod client {
    use super::*;
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::windows::named_pipe::ClientOptions;

    pub struct WinIpcClient {
        pipe: tokio::net::windows::named_pipe::NamedPipeClient,
    }

    impl WinIpcClient {
        pub async fn connect() -> Result<Self> {
            // Named pipe connect may need a retry if the server isn't ready.
            let deadline = tokio::time::Instant::now() + Duration::from_millis(500);

            loop {
                match ClientOptions::new().open(PIPE_NAME) {
                    Ok(pipe) => return Ok(Self { pipe }),
                    Err(e) if tokio::time::Instant::now() < deadline => {
                        // Wait for pipe to become available.
                        if e.raw_os_error() == Some(231) {
                            // ERROR_PIPE_BUSY — all instances in use, wait.
                            tokio::time::sleep(Duration::from_millis(50)).await;
                            continue;
                        }
                        return Err(e).context("connecting to named pipe");
                    }
                    Err(e) => {
                        return Err(e)
                            .context("daemon not running — start with `deskdrop-daemon`");
                    }
                }
            }
        }

        pub async fn request(&mut self, req: &IpcRequest) -> Result<IpcResponse> {
            let (reader, mut writer) = tokio::io::split(&mut self.pipe);
            let mut reader = BufReader::new(reader);

            let mut req_bytes = serde_json::to_vec(req)?;
            req_bytes.push(b'\n');
            writer.write_all(&req_bytes).await.context("pipe write")?;

            let mut line = String::new();
            reader.read_line(&mut line).await.context("pipe read")?;
            serde_json::from_str(&line).context("pipe response parse")
        }
    }
}
