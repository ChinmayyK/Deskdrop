//! C-compatible FFI layer.
//!
//! Platform wrappers load the shared library and call these functions.
//! All heap-allocated strings/bytes are freed via the corresponding
//! `deskdrop_free_*` functions — never call the system free() on them.
//!
//! Thread safety: all functions are safe to call from any thread.
//! The engine uses Tokio internally; we create a dedicated runtime here.

#![allow(clippy::missing_safety_doc)]

use crate::engine::{Engine, EngineConfig, EngineEvent};
use crate::protocol::ClipboardContent;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::sync::OnceLock;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use whoami;

// ── Tokio runtime (singleton) ─────────────────────────────────────────────────

static RT: OnceLock<Runtime> = OnceLock::new();

fn runtime() -> &'static Runtime {
    RT.get_or_init(|| Runtime::new().expect("Tokio runtime"))
}

// ── Engine handle ─────────────────────────────────────────────────────────────

pub struct DeskdropHandle {
    engine: Engine,
    event_rx: mpsc::Receiver<EngineEvent>,
}

/// Allocate and start the engine. Returns NULL on failure.
///
/// # Parameters
/// - `device_name`: UTF-8 C string; use NULL for auto-detected hostname.
/// - `port`: 0 → use default port (47823).
#[no_mangle]
pub unsafe extern "C" fn deskdrop_start(
    device_name: *const c_char,
    port: u16,
) -> *mut DeskdropHandle {
    let name = if device_name.is_null() {
        whoami::devicename()
    } else {
        unsafe { CStr::from_ptr(device_name) }
            .to_string_lossy()
            .into_owned()
    };

    let port = if port == 0 {
        crate::protocol::DEFAULT_PORT
    } else {
        port
    };

    let config = EngineConfig {
        device_name: name,
        port,
        ..EngineConfig::default()
    };

    let (event_tx, event_rx) = mpsc::channel(256);

    match runtime().block_on(Engine::start(config, event_tx)) {
        Ok(engine) => {
            #[cfg(windows)]
            {
                let e_clone = std::sync::Arc::new(engine.clone());
                runtime().spawn(async move {
                    let handler = std::sync::Arc::new(move |req: crate::ipc::IpcRequest| {
                        let eng = e_clone.clone();
                        async move {
                            match req {
                                crate::ipc::IpcRequest::Status => {
                                    let snap = eng.status_snapshot().await;
                                    let fp = eng.local_fingerprint();
                                    let peer_count = snap.peers.iter().filter(|p| p.status == crate::peer_manager::PeerConnectionState::Connected).count();
                                    crate::ipc::IpcResponse::ok(serde_json::json!({
                                        "peers": snap.peers,
                                        "peer_count": peer_count,
                                        "local_fingerprint": fp,
                                        "bind_ip": snap.bind_address.ip().to_string(),
                                        "bind_port": snap.bind_address.port(),
                                    }))
                                }
                                crate::ipc::IpcRequest::DisconnectPeer { device_id } => {
                                    if let Ok(id) = uuid::Uuid::parse_str(&device_id) {
                                        let _ = eng.disconnect_peer(id).await;
                                    }
                                    crate::ipc::IpcResponse::ok_empty()
                                }
                                crate::ipc::IpcRequest::ConnectPeer { ip, port } => {
                                    let _ = eng.connect_to_peer(ip, port).await;
                                    crate::ipc::IpcResponse::ok_empty()
                                }
                                crate::ipc::IpcRequest::RescanPeers => {
                                    eng.rescan_peers().await;
                                    crate::ipc::IpcResponse::ok_empty()
                                }
                                crate::ipc::IpcRequest::ConnectManual { host, port } => {
                                    let p = port.unwrap_or(47823);
                                    match eng.connect_to_peer(host, p).await {
                                        Ok(()) => crate::ipc::IpcResponse::ok_empty(),
                                        Err(e) => crate::ipc::IpcResponse::error(e.to_string()),
                                    }
                                }
                                crate::ipc::IpcRequest::SaveSettings { port, device_name, sync_enabled, sync_text, sync_images, sync_files, history_limit, max_history_text_bytes, max_payload_bytes, clipboard_poll_ms, max_pushes_per_sec, rate_limit_burst, smart_sync_duplicate_window_ms, smart_sync_debounce_ms, block_sensitive_text, require_tofu_confirmation, show_receive_notification, ignore_patterns } => {
                                    let patch = serde_json::json!({
                                        "port": port,
                                        "device_name": device_name,
                                        "sync_enabled": sync_enabled,
                                        "sync_text": sync_text,
                                        "sync_images": sync_images,
                                        "sync_files": sync_files,
                                        "history_limit": history_limit,
                                        "max_history_text_bytes": max_history_text_bytes,
                                        "max_payload_bytes": max_payload_bytes,
                                        "clipboard_poll_ms": clipboard_poll_ms,
                                        "max_pushes_per_sec": max_pushes_per_sec,
                                        "rate_limit_burst": rate_limit_burst,
                                        "smart_sync_duplicate_window_ms": smart_sync_duplicate_window_ms,
                                        "smart_sync_debounce_ms": smart_sync_debounce_ms,
                                        "block_sensitive_text": block_sensitive_text,
                                        "require_tofu_confirmation": require_tofu_confirmation,
                                        "show_receive_notification": show_receive_notification,
                                        "ignore_patterns": ignore_patterns,
                                    });
                                    let _ = eng.patch_settings(patch.to_string()).await;
                                    crate::ipc::IpcResponse::ok_empty()
                                }
                                crate::ipc::IpcRequest::PatchSettings { patch } => {
                                    let _ = eng.patch_settings(patch).await;
                                    crate::ipc::IpcResponse::ok_empty()
                                }
                                crate::ipc::IpcRequest::TrustPeer { device_id } => {
                                    if let Ok(id) = uuid::Uuid::parse_str(&device_id) {
                                        let _ = eng.trust_peer(id).await;
                                    }
                                    crate::ipc::IpcResponse::ok_empty()
                                }
                                crate::ipc::IpcRequest::RejectPeer { device_id } => {
                                    if let Ok(id) = uuid::Uuid::parse_str(&device_id) {
                                        let _ = eng.reject_peer(id).await;
                                    }
                                    crate::ipc::IpcResponse::ok_empty()
                                }
                                crate::ipc::IpcRequest::SendPairingRequest { device_id } => {
                                    if let Ok(id) = uuid::Uuid::parse_str(&device_id) {
                                        let _ = eng.send_pairing_request(id).await;
                                    }
                                    crate::ipc::IpcResponse::ok_empty()
                                }
                                crate::ipc::IpcRequest::RespondToPairing { device_id, accepted } => {
                                    if let Ok(id) = uuid::Uuid::parse_str(&device_id) {
                                        let _ = eng.respond_to_pairing(id, accepted).await;
                                    }
                                    crate::ipc::IpcResponse::ok_empty()
                                }
                                crate::ipc::IpcRequest::PauseSyncPeer { device_id } => {
                                    if let Ok(id) = uuid::Uuid::parse_str(&device_id) {
                                        let _ = eng.pause_sync_peer(id).await;
                                    }
                                    crate::ipc::IpcResponse::ok_empty()
                                }
                                crate::ipc::IpcRequest::ResumeSyncPeer { device_id } => {
                                    if let Ok(id) = uuid::Uuid::parse_str(&device_id) {
                                        let _ = eng.resume_sync_peer(id).await;
                                    }
                                    crate::ipc::IpcResponse::ok_empty()
                                }
                                crate::ipc::IpcRequest::ForgetDevice { device_id } => {
                                    if let Ok(id) = uuid::Uuid::parse_str(&device_id) {
                                        let _ = eng.forget_device(id).await;
                                    }
                                    crate::ipc::IpcResponse::ok_empty()
                                }
                                _ => crate::ipc::IpcResponse::error("unsupported in FFI IPC".to_string())
                            }
                        }
                    });
                    if let Err(e) = crate::ipc_windows::spawn_windows_ipc(handler).await {
                        eprintln!("Windows IPC server failed to start: {}", e);
                    }
                });
            }
            Box::into_raw(Box::new(DeskdropHandle { engine, event_rx }))
        }
        Err(e) => {
            eprintln!("deskdrop_start error: {:#}", e);
            std::ptr::null_mut()
        }
    }
}

/// Stop and free the engine.
///
/// # Safety
/// `handle` must be a pointer returned by `deskdrop_start` and must not be
/// used again after this call.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_stop(handle: *mut DeskdropHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

// ── Push clipboard ────────────────────────────────────────────────────────────

/// Push UTF-8 text to all peers. Returns number of peers reached.
///
/// # Safety
/// `text` must be a valid, non-null UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_push_text(
    handle: *mut DeskdropHandle,
    text: *const c_char,
) -> c_int {
    if handle.is_null() || text.is_null() {
        return -1;
    }
    let s = CStr::from_ptr(text).to_string_lossy().into_owned();
    let h = &*handle;
    runtime().block_on(h.engine.push_clipboard(ClipboardContent::Text(s))) as c_int
}

/// Push raw image bytes to all peers.
///
/// # Safety
/// `data` must point to `len` valid bytes; `mime` must be a valid C string.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_push_image(
    handle: *mut DeskdropHandle,
    mime: *const c_char,
    data: *const u8,
    len: usize,
) -> c_int {
    if handle.is_null() || mime.is_null() || data.is_null() {
        return -1;
    }
    let mime = CStr::from_ptr(mime).to_string_lossy().into_owned();
    let bytes = std::slice::from_raw_parts(data, len).to_vec();
    let h = &*handle;
    runtime().block_on(
        h.engine
            .push_clipboard(ClipboardContent::Image { mime, data: bytes }),
    ) as c_int
}

/// Push a file to all peers.
///
/// # Safety
/// `name` must be a valid C string; `data` must point to `len` valid bytes.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_push_file(
    handle: *mut DeskdropHandle,
    name: *const c_char,
    data: *const u8,
    len: usize,
) -> c_int {
    if handle.is_null() || name.is_null() || data.is_null() {
        return -1;
    }
    let name = CStr::from_ptr(name).to_string_lossy().into_owned();
    let bytes = std::slice::from_raw_parts(data, len).to_vec();
    let h = &*handle;
    runtime().block_on(
        h.engine
            .push_clipboard(ClipboardContent::File { name, data: bytes }),
    ) as c_int
}

#[no_mangle]
pub unsafe extern "C" fn deskdrop_send_file_path(
    handle: *mut DeskdropHandle,
    target_device_ptr: *const c_char,
    path_ptr: *const c_char,
    file_name_ptr: *const c_char,
    mime_type_ptr: *const c_char,
) -> c_int {
    if handle.is_null() || path_ptr.is_null() || file_name_ptr.is_null() || mime_type_ptr.is_null() {
        return -1;
    }
    
    let target_device = if !target_device_ptr.is_null() {
        let s = CStr::from_ptr(target_device_ptr).to_string_lossy();
        if s.is_empty() { None } else { uuid::Uuid::parse_str(&s).ok() }
    } else { None };

    let path = std::path::PathBuf::from(CStr::from_ptr(path_ptr).to_string_lossy().into_owned());
    let file_name = CStr::from_ptr(file_name_ptr).to_string_lossy().into_owned();
    let mime_type = CStr::from_ptr(mime_type_ptr).to_string_lossy().into_owned();

    let h = &*handle;
    let res = runtime().block_on(h.engine.send_file_path(path, file_name, mime_type, target_device));
    if res.is_ok() { 0 } else { -1 }
}

// ── Poll for events ───────────────────────────────────────────────────────────

/// Event type codes returned by `deskdrop_poll_event`.
pub const PB_EVENT_NONE: c_int = 0;
pub const PB_EVENT_CLIPBOARD_TEXT: c_int = 1;
pub const PB_EVENT_CLIPBOARD_IMAGE: c_int = 2;
pub const PB_EVENT_CLIPBOARD_FILE: c_int = 3;
pub const PB_EVENT_PAIRING_REQUESTED: c_int = 4;
pub const PB_EVENT_PEER_CONNECTED: c_int = 5;
pub const PB_EVENT_PEER_DISCONNECTED: c_int = 6;
pub const PB_EVENT_WARNING: c_int = 7;
pub const PB_EVENT_CLIPBOARD_SYNCED: c_int = 8;
pub const PB_EVENT_CLIPBOARD_AVAILABLE: c_int = 11; // timeline-first: not yet applied
pub const PB_EVENT_FILE_TRANSFER_INCOMING: c_int = 12;
pub const PB_EVENT_FILE_TRANSFER_PROGRESS: c_int = 13;
pub const PB_EVENT_FILE_TRANSFER_COMPLETE: c_int = 14;
pub const PB_EVENT_FILE_TRANSFER_FAILED: c_int = 15;
pub const PB_EVENT_ACTIVITY_UPDATED: c_int = 16;
pub const PB_EVENT_CALL_STATE_CHANGED: c_int = 17;
pub const PB_EVENT_CALL_ACTION: c_int = 18;
pub const PB_EVENT_BATTERY_STATE_CHANGED: c_int = 19;
pub const PB_EVENT_FILE_TRANSFER_PAUSED: c_int = 20;
pub const PB_EVENT_FILE_TRANSFER_RESUMED: c_int = 21;
pub const PB_EVENT_CAMERA_STREAM_REQUEST: c_int = 22;
pub const PB_EVENT_CAMERA_STREAM_ACCEPT: c_int = 23;
pub const PB_EVENT_CAMERA_STREAM_STOP: c_int = 24;
pub const PB_EVENT_CAMERA_FRAME: c_int = 25;
pub const PB_EVENT_SYSTEM_HEALTH_UPDATED: c_int = 26;

/// Opaque event payload. Call `deskdrop_event_*` accessors to read fields.
/// Must be freed with `deskdrop_free_event`.
pub struct PbEvent {
    inner: EngineEvent,
    // Cached C-string allocations for accessors.
    cached_str: Option<CString>,
    _cached_bytes: Option<Vec<u8>>,
    cached_mime: Option<CString>,
    cached_name: Option<CString>,
    cached_path: Option<CString>,
}

/// Non-blocking poll. Returns a heap-allocated `PbEvent*` or NULL if no event.
///
/// # Safety
/// `handle` must be valid.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_poll_event(handle: *mut DeskdropHandle) -> *mut PbEvent {
    if handle.is_null() {
        return std::ptr::null_mut();
    }
    let h = &mut *handle;
    match h.event_rx.try_recv() {
        Ok(event) => Box::into_raw(Box::new(PbEvent {
            inner: event,
            cached_str: None,
            _cached_bytes: None,
            cached_mime: None,
            cached_name: None,
            cached_path: None,
        })),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Returns the event type code for `event`.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_event_type(event: *const PbEvent) -> c_int {
    if event.is_null() {
        return PB_EVENT_NONE;
    }
    match &(*event).inner {
        EngineEvent::ClipboardReceived {
            content,
            auto_applied,
            ..
        } => {
            if *auto_applied {
                match &**content {
                    ClipboardContent::Text(_) => PB_EVENT_CLIPBOARD_TEXT,
                    ClipboardContent::Image { .. } => PB_EVENT_CLIPBOARD_IMAGE,
                    ClipboardContent::File { .. } => PB_EVENT_CLIPBOARD_FILE,
                }
            } else {
                // Timeline-first: available but not auto-applied.
                PB_EVENT_CLIPBOARD_AVAILABLE
            }
        }
        EngineEvent::HistoryMetadataReceived { .. } => PB_EVENT_WARNING,
        EngineEvent::SystemHealthUpdated(_) => PB_EVENT_SYSTEM_HEALTH_UPDATED,
        EngineEvent::ClipboardDeliveryStatus { .. } => PB_EVENT_WARNING,
        EngineEvent::PairingRequested { .. } => PB_EVENT_PAIRING_REQUESTED,
        EngineEvent::PairingConfirmed { .. } => PB_EVENT_WARNING,
        EngineEvent::PairingRejected { .. } => PB_EVENT_WARNING,
        EngineEvent::PairingRequest { .. } => PB_EVENT_WARNING,
        EngineEvent::PairingResponse { .. } => PB_EVENT_WARNING,
        EngineEvent::ClipboardSynced { .. } => PB_EVENT_CLIPBOARD_SYNCED,
        EngineEvent::ClipboardSyncFailed { .. } => PB_EVENT_WARNING,
        EngineEvent::PeerConnected { .. } => PB_EVENT_PEER_CONNECTED,
        EngineEvent::PeerDisconnected { .. } => PB_EVENT_PEER_DISCONNECTED,
        EngineEvent::FileTransferIncoming { .. } => PB_EVENT_FILE_TRANSFER_INCOMING,
        EngineEvent::FileTransferProgress { .. } => PB_EVENT_FILE_TRANSFER_PROGRESS,
        EngineEvent::FileTransferComplete { .. } => PB_EVENT_FILE_TRANSFER_COMPLETE,
        EngineEvent::FileTransferFailed { .. } => PB_EVENT_FILE_TRANSFER_FAILED,
        EngineEvent::FileTransferPaused { .. } => PB_EVENT_FILE_TRANSFER_PAUSED,
        EngineEvent::FileTransferResumed { .. } => PB_EVENT_FILE_TRANSFER_RESUMED,
        EngineEvent::ActivityFeedUpdated { .. } => PB_EVENT_ACTIVITY_UPDATED,
        EngineEvent::CallStateChanged { .. } => PB_EVENT_CALL_STATE_CHANGED,
        EngineEvent::CallActionRequest { .. } => PB_EVENT_CALL_ACTION,
        EngineEvent::BatteryStateChanged { .. } => PB_EVENT_BATTERY_STATE_CHANGED,
        EngineEvent::NotificationReceived { .. } => PB_EVENT_ACTIVITY_UPDATED,
        EngineEvent::CameraStreamRequest { .. } => PB_EVENT_CAMERA_STREAM_REQUEST,
        EngineEvent::CameraStreamAccept { .. } => PB_EVENT_CAMERA_STREAM_ACCEPT,
        EngineEvent::CameraStreamStop { .. } => PB_EVENT_CAMERA_STREAM_STOP,
        EngineEvent::CameraFrameReceived { .. } => PB_EVENT_CAMERA_FRAME,
        EngineEvent::Warning(_) => PB_EVENT_WARNING,
    }
}

/// Get the text payload (for TEXT events). Lifetime: until `deskdrop_free_event`.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_event_text(event: *mut PbEvent) -> *const c_char {
    let e = &mut *event;
    if let EngineEvent::ClipboardReceived {
        content,
        ..
    } = &e.inner
    {
        if let ClipboardContent::Text(ref s) = **content {
            let cs = CString::new(s.as_bytes()).unwrap_or_default();
            e.cached_str = Some(cs);
            return e.cached_str.as_ref().unwrap().as_ptr();
        }
    }
    std::ptr::null()
}

/// Get the device name associated with the event.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_event_device_name(event: *mut PbEvent) -> *const c_char {
    let e = &mut *event;
    let name: Option<&str> = match &e.inner {
        EngineEvent::ClipboardReceived { from_name, .. } => Some(from_name.as_str()),
        EngineEvent::HistoryMetadataReceived { from_name, .. } => Some(from_name.as_str()),
        EngineEvent::ClipboardSynced { peer_name, .. } => Some(peer_name.as_str()),
        EngineEvent::ClipboardSyncFailed { peer_name, .. } => Some(peer_name.as_str()),
        EngineEvent::PairingRequested { device_name, .. } => Some(device_name.as_str()),
        EngineEvent::PeerConnected { device_name, .. } => Some(device_name.as_str()),
        EngineEvent::FileTransferIncoming { from_name, .. } => Some(from_name.as_str()),
        EngineEvent::FileTransferComplete { from_name, .. } => Some(from_name.as_str()),
        _ => None,
    };
    if let Some(n) = name {
        let cs = CString::new(n).unwrap_or_default();
        e.cached_name = Some(cs);
        e.cached_name.as_ref().unwrap().as_ptr()
    } else {
        std::ptr::null()
    }
}

/// Returns 1 if this ClipboardReceived was auto-applied; 0 if timeline-first.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_event_auto_applied(event: *const PbEvent) -> c_int {
    if event.is_null() {
        return 0;
    }
    if let EngineEvent::ClipboardReceived { auto_applied, .. } = &(*event).inner {
        if *auto_applied {
            1
        } else {
            0
        }
    } else {
        0
    }
}

/// Returns the activity feed entry ID for a ClipboardReceived event (-1 if not applicable).
#[no_mangle]
pub unsafe extern "C" fn deskdrop_event_activity_id(event: *const PbEvent) -> i64 {
    if event.is_null() {
        return -1;
    }
    if let EngineEvent::ClipboardReceived { activity_id, .. } = &(*event).inner {
        *activity_id as i64
    } else {
        -1
    }
}

/// Get the transfer ID (hex string) for file transfer events.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_event_transfer_id(event: *mut PbEvent) -> *const c_char {
    if event.is_null() {
        return std::ptr::null();
    }
    let e = &mut *event;
    let tid = match &e.inner {
        EngineEvent::FileTransferIncoming { transfer_id, .. }
        | EngineEvent::FileTransferProgress { transfer_id, .. }
        | EngineEvent::FileTransferComplete { transfer_id, .. }
        | EngineEvent::FileTransferPaused { transfer_id, .. }
        | EngineEvent::FileTransferResumed { transfer_id, .. }
        | EngineEvent::FileTransferFailed { transfer_id, .. } => Some(hex::encode(transfer_id)),
        _ => None,
    };
    if let Some(s) = tid {
        let cs = CString::new(s).unwrap_or_default();
        e.cached_str = Some(cs);
        e.cached_str.as_ref().unwrap().as_ptr()
    } else {
        std::ptr::null()
    }
}

/// Get file name for file transfer events.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_event_transfer_file_name(event: *mut PbEvent) -> *const c_char {
    if event.is_null() {
        return std::ptr::null();
    }
    let e = &mut *event;
    let name = match &e.inner {
        EngineEvent::FileTransferIncoming { file_name, .. } => Some(file_name.as_str()),
        EngineEvent::FileTransferProgress { file_name, .. } => Some(file_name.as_str()),
        EngineEvent::FileTransferComplete { file_name, .. } => Some(file_name.as_str()),
        _ => None,
    };
    if let Some(n) = name {
        let cs = CString::new(n).unwrap_or_default();
        e.cached_mime = Some(cs);
        e.cached_mime.as_ref().unwrap().as_ptr()
    } else {
        std::ptr::null()
    }
}

/// Get progress percentage (0-100) for FileTransferProgress events; -1 otherwise.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_event_transfer_percent(event: *const PbEvent) -> c_int {
    if event.is_null() {
        return -1;
    }
    if let EngineEvent::FileTransferProgress { percent, .. } = &(*event).inner {
        *percent as c_int
    } else {
        -1
    }
}

/// Get total bytes for FileTransferIncoming/Progress events; -1 otherwise.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_event_transfer_total_bytes(event: *const PbEvent) -> i64 {
    if event.is_null() {
        return -1;
    }
    match &(*event).inner {
        EngineEvent::FileTransferIncoming { file_bytes, .. } => *file_bytes as i64,
        EngineEvent::FileTransferProgress { total_bytes, .. } => *total_bytes as i64,
        _ => -1,
    }
}

/// Get the destination path for FileTransferComplete events.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_event_transfer_dest_path(event: *mut PbEvent) -> *const c_char {
    if event.is_null() {
        return std::ptr::null();
    }
    let e = &mut *event;
    if let EngineEvent::FileTransferComplete { dest_path, .. } = &e.inner {
        let s = dest_path.to_string_lossy().into_owned();
        let cs = CString::new(s).unwrap_or_default();
        e.cached_path = Some(cs);
        e.cached_path.as_ref().unwrap().as_ptr()
    } else {
        std::ptr::null()
    }
}

/// Get the fingerprint display string for TOFU_PROMPT events.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_event_fingerprint(event: *mut PbEvent) -> *const c_char {
    let e = &mut *event;
    if let EngineEvent::PairingRequested {
        pin,
        ..
    } = &e.inner
    {
        let cs = CString::new(pin.as_bytes()).unwrap_or_default();
        e.cached_mime = Some(cs);
        e.cached_mime.as_ref().unwrap().as_ptr()
    } else {
        std::ptr::null()
    }
}

/// Get the device ID string for TOFU_PROMPT events.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_event_device_id(event: *mut PbEvent) -> *const c_char {
    let e = &mut *event;
    if let EngineEvent::PairingRequested { device_id, .. } = &e.inner {
        let cs = CString::new(device_id.to_string()).unwrap_or_default();
        e.cached_str = Some(cs);
        e.cached_str.as_ref().unwrap().as_ptr()
    } else {
        std::ptr::null()
    }
}

/// Apply a remote clipboard item by its content hash. Returns 1 on success.
/// The Swift layer calls this when the user clicks "Apply" in the timeline view.
///
/// # Safety
/// `handle` and `hash_ptr` must be valid.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_apply_clipboard(
    handle: *mut DeskdropHandle,
    hash_ptr: *const c_char,
) -> c_int {
    if handle.is_null() || hash_ptr.is_null() {
        return 0;
    }
    let hash = match std::ffi::CStr::from_ptr(hash_ptr).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return 0,
    };
    let h = &*handle;
    match runtime().block_on(h.engine.apply_clipboard_by_hash(hash)) {
        Ok(true) => 1,
        _ => 0,
    }
}

/// Accept an incoming file transfer. Returns 1 on success.
///
/// # Safety
/// `handle` and `transfer_id_hex` must be valid.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_accept_file_transfer(
    handle: *mut DeskdropHandle,
    transfer_id_hex: *const c_char,
) -> c_int {
    if handle.is_null() || transfer_id_hex.is_null() {
        return 0;
    }
    let hex_str = match std::ffi::CStr::from_ptr(transfer_id_hex).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return 0,
    };
    let Ok(bytes) = hex::decode(&hex_str) else {
        return 0;
    };
    let Ok(tid): Result<[u8; 16], _> = bytes.try_into() else {
        return 0;
    };
    let h = &*handle;
    match runtime().block_on(h.engine.accept_file_transfer(tid)) {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

/// Reject an incoming file transfer.
///
/// # Safety
#[no_mangle]
pub unsafe extern "C" fn deskdrop_reject_file_transfer(
    handle: *mut DeskdropHandle,
    transfer_id_hex: *const c_char,
) -> c_int {
    if handle.is_null() || transfer_id_hex.is_null() {
        return 0;
    }
    let hex_str = match std::ffi::CStr::from_ptr(transfer_id_hex).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return 0,
    };
    let Ok(bytes) = hex::decode(&hex_str) else {
        return 0;
    };
    let Ok(tid): Result<[u8; 16], _> = bytes.try_into() else {
        return 0;
    };
    let h = &*handle;
    match runtime().block_on(h.engine.reject_file_transfer(tid, "user rejected".into())) {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

/// Cancel an active file transfer.
///
/// # Safety
#[no_mangle]
pub unsafe extern "C" fn deskdrop_cancel_file_transfer(
    handle: *mut DeskdropHandle,
    transfer_id_hex: *const c_char,
) -> c_int {
    if handle.is_null() || transfer_id_hex.is_null() {
        return 0;
    }
    let tid_str = match CStr::from_ptr(transfer_id_hex).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let tid = if let Ok(parsed) = crate::ipc::parse_transfer_id(tid_str) {
        parsed
    } else {
        return 0;
    };
    let h = &*handle;
    match runtime().block_on(h.engine.cancel_file_transfer(tid)) {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

/// Pause an active file transfer.
///
/// # Safety
#[no_mangle]
pub unsafe extern "C" fn deskdrop_pause_file_transfer(
    handle: *mut DeskdropHandle,
    transfer_id_hex: *const c_char,
) -> c_int {
    if handle.is_null() || transfer_id_hex.is_null() {
        return 0;
    }
    let tid_str = match CStr::from_ptr(transfer_id_hex).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let tid = if let Ok(parsed) = crate::ipc::parse_transfer_id(tid_str) {
        parsed
    } else {
        return 0;
    };
    let h = &*handle;
    match runtime().block_on(h.engine.pause_file_transfer(tid)) {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

/// Resume a paused file transfer.
///
/// # Safety
#[no_mangle]
pub unsafe extern "C" fn deskdrop_resume_file_transfer(
    handle: *mut DeskdropHandle,
    transfer_id_hex: *const c_char,
) -> c_int {
    if handle.is_null() || transfer_id_hex.is_null() {
        return 0;
    }
    let tid_str = match CStr::from_ptr(transfer_id_hex).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let tid = if let Ok(parsed) = crate::ipc::parse_transfer_id(tid_str) {
        parsed
    } else {
        return 0;
    };
    let h = &*handle;
    match runtime().block_on(h.engine.resume_file_transfer(tid)) {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

/// Free an event returned by `deskdrop_poll_event`.
///
/// # Safety
/// `event` must be a pointer returned by `deskdrop_poll_event`.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_free_event(event: *mut PbEvent) {
    if !event.is_null() {
        drop(Box::from_raw(event));
    }
}

// ── Camera accessors ─────────────────────────────────────────────────────────

/// Get the data buffer for a PB_EVENT_CAMERA_FRAME event.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_event_camera_frame_data(_event: *mut PbEvent) -> *const u8 {
    // Camera frames are no longer sent via event bus to avoid OOM.
    // They must be fetched directly from the engine state via deskdrop_engine_get_camera_frame.
    std::ptr::null()
}

/// Get the data length for a PB_EVENT_CAMERA_FRAME event.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_event_camera_frame_len(_event: *const PbEvent) -> usize {
    0
}

/// Fetch the latest camera frame for a specific peer directly from the engine.
/// Copies up to `max_len` bytes into `out_buffer`. Returns actual length, or 0 if none/error.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_engine_get_camera_frame(
    engine: *mut crate::engine::Engine,
    peer_id_bytes: *const u8,
    out_buffer: *mut u8,
    max_len: usize,
) -> usize {
    if engine.is_null() || peer_id_bytes.is_null() || out_buffer.is_null() {
        return 0;
    }
    let engine = &*engine;
    let peer_id = match uuid::Uuid::from_slice(std::slice::from_raw_parts(peer_id_bytes, 16)) {
        Ok(id) => id,
        Err(_) => return 0,
    };
    
    // Fetch frame from engine using public method
    if let Some(frame_data) = engine.get_latest_camera_frame(peer_id) {
        let len = std::cmp::min(frame_data.len(), max_len);
        std::ptr::copy_nonoverlapping(frame_data.as_ptr(), out_buffer, len);
        return len;
    }
    0
}

/// Push a camera frame to all peers.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_push_video_frame(
    handle: *mut DeskdropHandle,
    data: *const u8,
    len: usize,
) -> c_int {
    if handle.is_null() || data.is_null() || len == 0 {
        return -1;
    }
    let bytes = std::slice::from_raw_parts(data, len).to_vec();
    let h = &*handle;
    runtime().block_on(h.engine.push_camera_frame(bytes));
    0
}

// ── Windows P/Invoke helpers ──────────────────────────────────────────────────

/// Respond to a TOFU prompt.  `trust` = 1 to accept, 0 to reject.
/// Returns 0 on success.
///
/// # Safety
/// `handle` and `device_id_ptr` must be valid.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_trust_peer(
    handle: *mut DeskdropHandle,
    device_id_ptr: *const std::ffi::c_char,
    trust: std::ffi::c_int,
) -> std::ffi::c_int {
    if handle.is_null() || device_id_ptr.is_null() {
        return -1;
    }
    let id_str = match std::ffi::CStr::from_ptr(device_id_ptr).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -1,
    };
    let device_id = match uuid::Uuid::parse_str(&id_str) {
        Ok(id) => id,
        Err(_) => return -1,
    };
    let h = &*handle;
    if trust != 0 {
        runtime().block_on(async {
            let _ = h.engine.trust_peer(device_id).await;
        });
    } else {
        runtime().block_on(async {
            let _ = h.engine.reject_peer(device_id).await;
        });
    }
    0
}

/// Alias for `deskdrop_apply_clipboard` for Windows P/Invoke compatibility.
///
/// # Safety
/// `handle` and `hash_ptr` must be valid.
#[no_mangle]
pub unsafe extern "C" fn deskdrop_apply_by_hash(
    handle: *mut DeskdropHandle,
    hash_ptr: *const std::ffi::c_char,
) -> std::ffi::c_int {
    deskdrop_apply_clipboard(handle, hash_ptr)
}
