//! C-compatible FFI layer.
//!
//! Platform wrappers load the shared library and call these functions.
//! All heap-allocated strings/bytes are freed via the corresponding
//! `cliprelay_free_*` functions — never call the system free() on them.
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

// ── Tokio runtime (singleton) ─────────────────────────────────────────────────

static RT: OnceLock<Runtime> = OnceLock::new();

fn runtime() -> &'static Runtime {
    RT.get_or_init(|| Runtime::new().expect("Tokio runtime"))
}

// ── Engine handle ─────────────────────────────────────────────────────────────

pub struct ClipRelayHandle {
    engine: Engine,
    event_rx: mpsc::Receiver<EngineEvent>,
}

/// Allocate and start the engine. Returns NULL on failure.
///
/// # Parameters
/// - `device_name`: UTF-8 C string; use NULL for auto-detected hostname.
/// - `port`: 0 → use default port (47823).
#[no_mangle]
pub extern "C" fn cliprelay_start(device_name: *const c_char, port: u16) -> *mut ClipRelayHandle {
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
        Ok(engine) => Box::into_raw(Box::new(ClipRelayHandle { engine, event_rx })),
        Err(e) => {
            eprintln!("cliprelay_start error: {:#}", e);
            std::ptr::null_mut()
        }
    }
}

/// Stop and free the engine.
///
/// # Safety
/// `handle` must be a pointer returned by `cliprelay_start` and must not be
/// used again after this call.
#[no_mangle]
pub unsafe extern "C" fn cliprelay_stop(handle: *mut ClipRelayHandle) {
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
pub unsafe extern "C" fn cliprelay_push_text(
    handle: *mut ClipRelayHandle,
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
pub unsafe extern "C" fn cliprelay_push_image(
    handle: *mut ClipRelayHandle,
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
pub unsafe extern "C" fn cliprelay_push_file(
    handle: *mut ClipRelayHandle,
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

// ── Poll for events ───────────────────────────────────────────────────────────

/// Event type codes returned by `cliprelay_poll_event`.
pub const PB_EVENT_NONE: c_int = 0;
pub const PB_EVENT_CLIPBOARD_TEXT: c_int = 1;
pub const PB_EVENT_CLIPBOARD_IMAGE: c_int = 2;
pub const PB_EVENT_CLIPBOARD_FILE: c_int = 3;
pub const PB_EVENT_TOFU_PROMPT: c_int = 4;
pub const PB_EVENT_PEER_CONNECTED: c_int = 5;
pub const PB_EVENT_PEER_DISCONNECTED: c_int = 6;
pub const PB_EVENT_WARNING: c_int = 7;
pub const PB_EVENT_CLIPBOARD_SYNCED: c_int = 8;

/// Opaque event payload. Call `cliprelay_event_*` accessors to read fields.
/// Must be freed with `cliprelay_free_event`.
pub struct PbEvent {
    inner: EngineEvent,
    // Cached C-string allocations for accessors.
    cached_str: Option<CString>,
    cached_bytes: Option<Vec<u8>>,
    cached_mime: Option<CString>,
    cached_name: Option<CString>,
}

/// Non-blocking poll. Returns a heap-allocated `PbEvent*` or NULL if no event.
///
/// # Safety
/// `handle` must be valid.
#[no_mangle]
pub unsafe extern "C" fn cliprelay_poll_event(handle: *mut ClipRelayHandle) -> *mut PbEvent {
    if handle.is_null() {
        return std::ptr::null_mut();
    }
    let h = &mut *handle;
    match h.event_rx.try_recv() {
        Ok(event) => Box::into_raw(Box::new(PbEvent {
            inner: event,
            cached_str: None,
            cached_bytes: None,
            cached_mime: None,
            cached_name: None,
        })),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Returns the event type code for `event`.
#[no_mangle]
pub unsafe extern "C" fn cliprelay_event_type(event: *const PbEvent) -> c_int {
    if event.is_null() {
        return PB_EVENT_NONE;
    }
    match &(*event).inner {
        EngineEvent::ClipboardReceived { content, .. } => match content {
            ClipboardContent::Text(_) => PB_EVENT_CLIPBOARD_TEXT,
            ClipboardContent::Image { .. } => PB_EVENT_CLIPBOARD_IMAGE,
            ClipboardContent::File { .. } => PB_EVENT_CLIPBOARD_FILE,
        },
        EngineEvent::HistoryMetadataReceived { .. } => PB_EVENT_WARNING,
        EngineEvent::TofuPrompt { .. } => PB_EVENT_TOFU_PROMPT,
        EngineEvent::ClipboardSynced { .. } => PB_EVENT_CLIPBOARD_SYNCED,
        EngineEvent::ClipboardSyncFailed { .. } => PB_EVENT_WARNING,
        EngineEvent::PeerConnected { .. } => PB_EVENT_PEER_CONNECTED,
        EngineEvent::PeerDisconnected { .. } => PB_EVENT_PEER_DISCONNECTED,
        EngineEvent::Warning(_) => PB_EVENT_WARNING,
    }
}

/// Get the text payload (for TEXT events). Lifetime: until `cliprelay_free_event`.
#[no_mangle]
pub unsafe extern "C" fn cliprelay_event_text(event: *mut PbEvent) -> *const c_char {
    let e = &mut *event;
    if let EngineEvent::ClipboardReceived {
        content: ClipboardContent::Text(ref s),
        ..
    } = e.inner
    {
        let cs = CString::new(s.as_bytes()).unwrap_or_default();
        e.cached_str = Some(cs);
        e.cached_str.as_ref().unwrap().as_ptr()
    } else {
        std::ptr::null()
    }
}

/// Get the device name associated with the event.
#[no_mangle]
pub unsafe extern "C" fn cliprelay_event_device_name(event: *mut PbEvent) -> *const c_char {
    let e = &mut *event;
    let name: Option<&str> = match &e.inner {
        EngineEvent::ClipboardReceived { from_name, .. } => Some(from_name.as_str()),
        EngineEvent::HistoryMetadataReceived { from_name, .. } => Some(from_name.as_str()),
        EngineEvent::ClipboardSynced { peer_name, .. } => Some(peer_name.as_str()),
        EngineEvent::ClipboardSyncFailed { peer_name, .. } => Some(peer_name.as_str()),
        EngineEvent::TofuPrompt { device_name, .. } => Some(device_name.as_str()),
        EngineEvent::PeerConnected { device_name, .. } => Some(device_name.as_str()),
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

/// Get the fingerprint display string for TOFU_PROMPT events.
#[no_mangle]
pub unsafe extern "C" fn cliprelay_event_fingerprint(event: *mut PbEvent) -> *const c_char {
    let e = &mut *event;
    if let EngineEvent::TofuPrompt {
        fingerprint_display,
        ..
    } = &e.inner
    {
        let cs = CString::new(fingerprint_display.as_bytes()).unwrap_or_default();
        e.cached_mime = Some(cs);
        e.cached_mime.as_ref().unwrap().as_ptr()
    } else {
        std::ptr::null()
    }
}

/// Free an event returned by `cliprelay_poll_event`.
///
/// # Safety
/// `event` must be a pointer returned by `cliprelay_poll_event`.
#[no_mangle]
pub unsafe extern "C" fn cliprelay_free_event(event: *mut PbEvent) {
    if !event.is_null() {
        drop(Box::from_raw(event));
    }
}
