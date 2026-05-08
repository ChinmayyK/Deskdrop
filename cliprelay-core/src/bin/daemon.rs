use anyhow::{anyhow, Context, Result};
use cliprelay_core::{
    engine::{Engine, EngineConfig, EngineEvent, SyncDispatchReport, SyncTarget},
    history::{History, HistoryEntry},
    ipc::{IpcRequest, IpcResponse},
    peer_manager::PeerConnectionState,
    protocol::ClipboardContent,
    settings::{default_history_path, default_settings_path, SettingsStore},
    trust::format_fingerprint,
};
use serde::Serialize;
use serde_json::json;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, Mutex, Notify};
use uuid::Uuid;

const MAX_FEEDBACK_EVENTS: usize = 200;
const MAX_INCOMING_CLIPBOARDS: usize = 128;

#[derive(Debug, Clone, Serialize)]
struct FeedbackEvent {
    timestamp: u64,
    kind: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    clipboard_id: Option<u64>,
}

#[derive(Clone)]
struct DaemonState {
    engine: Arc<Engine>,
    settings: Arc<Mutex<SettingsStore>>,
    history: Arc<Mutex<History>>,
    feedback: Arc<Mutex<VecDeque<FeedbackEvent>>>,
    incoming_clipboards: Arc<Mutex<HashMap<u64, serde_json::Value>>>,
    incoming_order: Arc<Mutex<VecDeque<u64>>>,
    started_at: Instant,
    shutdown: Arc<Notify>,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("ClipRelay daemon failed: {error:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("CLIPRELAY_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let settings_store = SettingsStore::load(default_settings_path()).context("loading settings")?;
    let initial_settings = settings_store.get().clone();

    let mut config = EngineConfig::default();
    config.device_name = initial_settings.resolved_device_name();
    config.port = initial_settings.port;

    let (event_tx, mut event_rx) = mpsc::channel(256);
    let engine = Arc::new(Engine::start(config, event_tx).await?);
    engine.apply_settings(initial_settings.clone()).await;

    let history = History::load_with_limit(
        default_history_path(),
        initial_settings.effective_history_limit(),
    )
    .context("loading history")?;

    let state = DaemonState {
        engine: engine.clone(),
        settings: Arc::new(Mutex::new(settings_store)),
        history: Arc::new(Mutex::new(history)),
        feedback: Arc::new(Mutex::new(VecDeque::new())),
        incoming_clipboards: Arc::new(Mutex::new(HashMap::new())),
        incoming_order: Arc::new(Mutex::new(VecDeque::new())),
        started_at: Instant::now(),
        shutdown: Arc::new(Notify::new()),
    };

    let event_state = state.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            if let Err(error) = handle_event(event_state.clone(), event).await {
                tracing::warn!("event processing failed: {error:#}");
            }
        }
    });

    #[cfg(unix)]
    {
        let handler_state = state.clone();
        cliprelay_core::ipc::server::spawn(Arc::new(move |req| {
            let handler_state = handler_state.clone();
            async move { handle_request(handler_state, req).await }
        }))
        .await
        .context("starting IPC server")?;
    }

    tracing::info!(
        "ClipRelay daemon started. IPC socket: {:?}",
        cliprelay_core::ipc::socket_path()
    );

    tokio::select! {
        _ = state.shutdown.notified() => {
            tracing::info!("Shutdown requested by IPC client");
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Shutting down on SIGINT");
        }
    }

    Ok(())
}

async fn handle_event(state: DaemonState, event: EngineEvent) -> Result<()> {
    match event {
        EngineEvent::ClipboardReceived {
            from_name,
            content,
            activity_id,
            ..
        } => {
            let settings = state.settings.lock().await.get().clone();
            {
                let mut history = state.history.lock().await;
                history.push_with_options(
                    &content,
                    from_name.clone(),
                    settings.max_history_text_bytes,
                )?;
            }

            store_incoming_clipboard(&state, activity_id, incoming_payload_json(activity_id, &content)).await;
            push_feedback(
                &state,
                FeedbackEvent {
                    timestamp: now_secs(),
                    kind: "clipboard_received".into(),
                    message: format!("Clipboard received from {from_name}"),
                    device_id: None,
                    device_name: Some(from_name),
                    clipboard_id: Some(activity_id),
                },
            )
            .await;
        }
        EngineEvent::HistoryMetadataReceived {
            from_name, entry, ..
        } => {
            state.history.lock().await.push_metadata(&entry)?;
            push_feedback(
                &state,
                FeedbackEvent {
                    timestamp: now_secs(),
                    kind: "history_metadata".into(),
                    message: format!("History updated from {from_name}"),
                    device_id: None,
                    device_name: Some(from_name),
                    clipboard_id: None,
                },
            )
            .await;
        }
        EngineEvent::ClipboardSynced {
            peer_device,
            peer_name,
            ..
        } => {
            push_feedback(
                &state,
                FeedbackEvent {
                    timestamp: now_secs(),
                    kind: "clipboard_dispatch".into(),
                    message: format!("Sent clipboard to {peer_name}"),
                    device_id: Some(peer_device.to_string()),
                    device_name: Some(peer_name),
                    clipboard_id: None,
                },
            )
            .await;
        }
        EngineEvent::ClipboardSyncFailed {
            peer_device,
            peer_name,
            reason,
            ..
        } => {
            push_feedback(
                &state,
                FeedbackEvent {
                    timestamp: now_secs(),
                    kind: "clipboard_sync_failed".into(),
                    message: format!("Failed to sync with {peer_name}: {reason}"),
                    device_id: Some(peer_device.to_string()),
                    device_name: Some(peer_name),
                    clipboard_id: None,
                },
            )
            .await;
        }
        EngineEvent::TofuPrompt {
            device_id,
            device_name,
            ..
        } => {
            push_feedback(
                &state,
                FeedbackEvent {
                    timestamp: now_secs(),
                    kind: "trust_prompt".into(),
                    message: format!("{device_name} wants to connect"),
                    device_id: Some(device_id.to_string()),
                    device_name: Some(device_name),
                    clipboard_id: None,
                },
            )
            .await;
        }
        EngineEvent::PeerConnected {
            device_id,
            device_name,
            ..
        } => {
            push_feedback(
                &state,
                FeedbackEvent {
                    timestamp: now_secs(),
                    kind: "peer_connected".into(),
                    message: format!("{device_name} connected"),
                    device_id: Some(device_id.to_string()),
                    device_name: Some(device_name),
                    clipboard_id: None,
                },
            )
            .await;
        }
        EngineEvent::PeerDisconnected {
            device_id,
            device_name,
            reason,
        } => {
            let name = device_name.unwrap_or_else(|| "Unknown device".into());
            let detail = reason
                .as_deref()
                .map(|value| format!(" ({value})"))
                .unwrap_or_default();
            push_feedback(
                &state,
                FeedbackEvent {
                    timestamp: now_secs(),
                    kind: "peer_disconnected".into(),
                    message: format!("{name} disconnected{detail}"),
                    device_id: Some(device_id.to_string()),
                    device_name: Some(name),
                    clipboard_id: None,
                },
            )
            .await;
        }
        EngineEvent::Warning(message) => {
            push_feedback(
                &state,
                FeedbackEvent {
                    timestamp: now_secs(),
                    kind: "warning".into(),
                    message,
                    device_id: None,
                    device_name: None,
                    clipboard_id: None,
                },
            )
            .await;
        }
        _ => {}
    }

    Ok(())
}

async fn handle_request(state: DaemonState, req: IpcRequest) -> IpcResponse {
    match handle_request_inner(state, req).await {
        Ok(response) => response,
        Err(error) => IpcResponse::error(error.to_string()),
    }
}

async fn handle_request_inner(state: DaemonState, req: IpcRequest) -> Result<IpcResponse> {
    match req {
        IpcRequest::Ping => Ok(IpcResponse::Pong {
            uptime_secs: state.started_at.elapsed().as_secs(),
        }),
        IpcRequest::Status => {
            let snapshot = state.engine.status_snapshot().await;
            let settings = state.settings.lock().await.get().clone();
            let peer_count = snapshot
                .peers
                .iter()
                .filter(|peer| peer.status == PeerConnectionState::Connected)
                .count();
            Ok(IpcResponse::ok(json!({
                "device_name": settings.resolved_device_name(),
                "port": settings.port,
                "sync_enabled": settings.sync_enabled,
                "peer_count": peer_count,
                "last_sync_at": snapshot.last_sync_at,
                "peers": snapshot.peers,
            })))
        }
        IpcRequest::Peers => {
            let snapshot = state.engine.status_snapshot().await;
            Ok(IpcResponse::ok(snapshot.peers))
        }
        IpcRequest::TrustedDevices => Ok(IpcResponse::ok(state.engine.trusted_devices().await)),
        IpcRequest::DeviceDetails { device_id } => {
            let record = state
                .engine
                .trusted_devices()
                .await
                .into_iter()
                .find(|device| device.device_id == parse_uuid(&device_id).unwrap_or_default())
                .context("device not found")?;
            Ok(IpcResponse::ok(json!({
                "device_id": record.device_id,
                "device_name": record.device_name,
                "display_name": record.display_name,
                "effective_name": record.effective_name(),
                "state": record.state,
                "fingerprint": format_fingerprint(&record.key_fingerprint),
                "first_seen": record.first_seen,
                "trusted_since": record.trusted_since,
                "last_seen": record.last_seen,
            })))
        }
        IpcRequest::ConnectPeer { ip, port } => {
            state.engine.connect_to_peer(ip, port).await?;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::DisconnectPeer { device_id } => {
            state.engine.disconnect_peer(parse_uuid(&device_id)?).await?;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::TrustPeer { device_id } => {
            state.engine.trust_peer(parse_uuid(&device_id)?).await?;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::RejectPeer { device_id } => {
            state.engine.reject_peer(parse_uuid(&device_id)?).await?;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::RevokeTrustedDevice { device_id } => {
            state.engine.revoke_peer(parse_uuid(&device_id)?).await?;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::RenameTrustedDevice {
            device_id,
            display_name,
        } => {
            state
                .engine
                .rename_trusted_device(parse_uuid(&device_id)?, display_name)
                .await?;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::PauseSyncPeer { device_id } => {
            state.engine.pause_sync_peer(parse_uuid(&device_id)?).await?;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::ResumeSyncPeer { device_id } => {
            state.engine.resume_sync_peer(parse_uuid(&device_id)?).await?;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::ForgetDevice { device_id } => {
            state.engine.forget_device(parse_uuid(&device_id)?).await?;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::SetAutoConnect { device_id, enabled } => {
            state
                .engine
                .set_auto_connect(parse_uuid(&device_id)?, enabled)
                .await?;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::PushText { text } => Ok(IpcResponse::ok(dispatch_text(&state, text, SyncTarget::All).await?)),
        IpcRequest::PushTextTo { text, target } => Ok(IpcResponse::ok(
            dispatch_text(&state, text, SyncTarget::Device(parse_uuid(&target)?)).await?,
        )),
        IpcRequest::PushImage { mime, data_base64 } => {
            let data = decode_base64(&data_base64)?;
            let content = ClipboardContent::Image {
                mime: mime.clone(),
                data,
            };
            remember_history(&state, &content, current_device_name(&state).await).await?;
            Ok(IpcResponse::ok(state.engine.push_clipboard_to(content, SyncTarget::All).await))
        }
        IpcRequest::PushFile { name, data_base64 } => {
            let data = decode_base64(&data_base64)?;
            let content = ClipboardContent::File {
                name: name.clone(),
                data,
            };
            remember_history(&state, &content, current_device_name(&state).await).await?;
            Ok(IpcResponse::ok(state.engine.push_clipboard_to(content, SyncTarget::All).await))
        }
        IpcRequest::RememberText { text } => {
            let content = ClipboardContent::Text(text);
            Ok(IpcResponse::ok(
                remember_history(&state, &content, current_device_name(&state).await).await?,
            ))
        }
        IpcRequest::History { last } => {
            let history = state.history.lock().await;
            Ok(IpcResponse::ok(history.recent(last).cloned().collect::<Vec<_>>()))
        }
        IpcRequest::HistorySearch { query, limit } => {
            let history = state.history.lock().await;
            Ok(IpcResponse::ok(
                history.search(&query).take(limit).cloned().collect::<Vec<_>>(),
            ))
        }
        IpcRequest::HistoryPin { id, pinned } => {
            let mut history = state.history.lock().await;
            let entry = history
                .set_pinned(id, pinned)?
                .cloned()
                .context("history item not found")?;
            Ok(IpcResponse::ok(entry))
        }
        IpcRequest::HistoryDelete { id } => {
            state.history.lock().await.remove(id)?;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::HistoryClear => {
            state.history.lock().await.clear()?;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::HistoryRepush { id, target } => {
            let target = target
                .map(|value| parse_uuid(&value))
                .transpose()?
                .map(SyncTarget::Device)
                .unwrap_or(SyncTarget::All);
            let entry = {
                let history = state.history.lock().await;
                history.get(id).cloned().context("history item not found")?
            };
            let text = entry
                .repushable_text()
                .map(str::to_owned)
                .context("only text history items can be re-sent right now")?;
            Ok(IpcResponse::ok(
                state
                    .engine
                    .push_clipboard_to(ClipboardContent::Text(text), target)
                    .await,
            ))
        }
        IpcRequest::Feedback { last } => {
            let feedback = state.feedback.lock().await;
            Ok(IpcResponse::ok(
                feedback.iter().rev().take(last).cloned().collect::<Vec<_>>(),
            ))
        }
        IpcRequest::IncomingClipboard { id } => {
            let payload = state
                .incoming_clipboards
                .lock()
                .await
                .get(&id)
                .cloned()
                .context("clipboard payload not found")?;
            Ok(IpcResponse::ok(payload))
        }
        IpcRequest::GetSettings => Ok(IpcResponse::ok(state.settings.lock().await.get().clone())),
        IpcRequest::PatchSettings { patch } => {
            let updated = {
                let mut store = state.settings.lock().await;
                store.patch(&patch)?;
                store.get().clone()
            };
            {
                let mut history = state.history.lock().await;
                history.set_max_entries(updated.effective_history_limit())?;
            }
            state.engine.apply_settings(updated).await;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::SetSyncEnabled { enabled } => {
            let updated = {
                let mut store = state.settings.lock().await;
                store.patch(&json!({ "sync_enabled": enabled }).to_string())?;
                store.get().clone()
            };
            state.engine.apply_settings(updated).await;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::ActivityRecent { limit } => Ok(IpcResponse::ok(state.engine.activity_recent(limit).await)),
        IpcRequest::ActivitySince { since_id } => Ok(IpcResponse::ok(state.engine.activity_since(since_id).await)),
        IpcRequest::PendingRemoteClipboards => Ok(IpcResponse::ok(state.engine.pending_remote_clipboards().await)),
        IpcRequest::ApplyClipboard { content_hash } => {
            state.engine.apply_clipboard_by_hash(content_hash).await?;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::SendFile {
            name,
            mime,
            data_base64,
            target_device,
        } => {
            let data = decode_base64(&data_base64)?;
            let transfer_id = state
                .engine
                .send_file(
                    data,
                    name,
                    mime,
                    target_device.as_deref().map(parse_uuid).transpose()?,
                )
                .await?;
            Ok(IpcResponse::ok(hex::encode(transfer_id)))
        }
        IpcRequest::AcceptFileTransfer { transfer_id } => {
            state
                .engine
                .accept_file_transfer(parse_transfer_id(&transfer_id)?)
                .await?;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::RejectFileTransfer { transfer_id, reason } => {
            state
                .engine
                .reject_file_transfer(parse_transfer_id(&transfer_id)?, reason)
                .await?;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::CancelFileTransfer { transfer_id } => {
            state
                .engine
                .cancel_file_transfer(parse_transfer_id(&transfer_id)?)
                .await?;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::SetTimelineFirstMode { enabled } => {
            let updated = {
                let mut store = state.settings.lock().await;
                store.patch(&json!({ "timeline_first_mode": enabled }).to_string())?;
                store.get().clone()
            };
            state.engine.apply_settings(updated).await;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::SetAutoApplyClipboard { enabled } => {
            let updated = {
                let mut store = state.settings.lock().await;
                store.patch(&json!({ "auto_apply_remote_clipboard": enabled }).to_string())?;
                store.get().clone()
            };
            state.engine.apply_settings(updated).await;
            Ok(IpcResponse::ok_empty())
        }
        IpcRequest::Shutdown => {
            state.shutdown.notify_waiters();
            Ok(IpcResponse::ok_empty())
        }
    }
}

async fn dispatch_text(
    state: &DaemonState,
    text: String,
    target: SyncTarget,
) -> Result<SyncDispatchReport> {
    let content = ClipboardContent::Text(text);
    remember_history(state, &content, current_device_name(state).await).await?;
    Ok(state.engine.push_clipboard_to(content, target).await)
}

async fn remember_history(
    state: &DaemonState,
    content: &ClipboardContent,
    source_device: String,
) -> Result<HistoryEntry> {
    let settings = state.settings.lock().await.get().clone();
    let mut history = state.history.lock().await;
    history
        .push_with_options(content, source_device, settings.max_history_text_bytes)
        .map(|entry| entry.clone())
}

async fn current_device_name(state: &DaemonState) -> String {
    state.settings.lock().await.get().resolved_device_name()
}

async fn push_feedback(state: &DaemonState, event: FeedbackEvent) {
    let mut feedback = state.feedback.lock().await;
    feedback.push_back(event);
    while feedback.len() > MAX_FEEDBACK_EVENTS {
        feedback.pop_front();
    }
}

async fn store_incoming_clipboard(state: &DaemonState, id: u64, payload: serde_json::Value) {
    state.incoming_clipboards.lock().await.insert(id, payload);
    let mut order = state.incoming_order.lock().await;
    order.push_back(id);
    while order.len() > MAX_INCOMING_CLIPBOARDS {
        if let Some(oldest) = order.pop_front() {
            state.incoming_clipboards.lock().await.remove(&oldest);
        }
    }
}

fn incoming_payload_json(id: u64, content: &ClipboardContent) -> serde_json::Value {
    match content {
        ClipboardContent::Text(text) => json!({
            "id": id,
            "type": "text",
            "text": text,
        }),
        ClipboardContent::Image { mime, data } => json!({
            "id": id,
            "type": "image",
            "mime": mime,
            "data_base64": base64::engine::general_purpose::STANDARD.encode(data),
        }),
        ClipboardContent::File { name, data } => json!({
            "id": id,
            "type": "file",
            "name": name,
            "data_base64": base64::engine::general_purpose::STANDARD.encode(data),
        }),
    }
}

fn parse_uuid(value: &str) -> Result<Uuid> {
    Uuid::parse_str(value).with_context(|| format!("invalid UUID: {value}"))
}

fn decode_base64(value: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(value).map_err(|error| anyhow!("invalid base64 payload: {error}"))
}

fn parse_transfer_id(value: &str) -> Result<[u8; 16]> {
    let bytes = hex::decode(value).with_context(|| format!("invalid transfer id: {value}"))?;
    bytes.try_into().map_err(|_| anyhow!("transfer id must be 16 bytes"))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
