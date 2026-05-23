//! JNI bridge for Android.
//!
//! Exposes the same Deskdrop engine to Kotlin/Java via JNI.
//! This file lives alongside the other Rust sources and is compiled
//! into libcliprelay_core.so for each Android ABI.
//!
//! Generated JNI signatures match the Kotlin declarations in
//! ClipRelayJni.kt (package com.cliprelay, object ClipRelayJni).

#![cfg(target_os = "android")]

use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jboolean, jbyteArray, jint, jlong, jstring};
use jni::JNIEnv;

use crate::engine::{Engine, EngineConfig};
use crate::protocol::ClipboardContent;
use std::path::PathBuf;
use std::sync::OnceLock;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

static RT: OnceLock<Runtime> = OnceLock::new();
fn rt() -> &'static Runtime {
    RT.get_or_init(|| Runtime::new().expect("Tokio runtime"))
}

struct AndroidHandle {
    engine: Engine,
    event_rx: mpsc::Receiver<crate::engine::EngineEvent>,
}

// ── start ─────────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_start(
    mut env: JNIEnv,
    _class: JClass,
    device_name: JString,
    port: jint,
    data_dir: JString,
    file_save_dir: JString,
) -> jlong {
    let name: String = env
        .get_string(&device_name)
        .map(|s| s.into())
        .unwrap_or_else(|_| whoami::devicename());
    let data_root = env
        .get_string(&data_dir)
        .ok()
        .map(|s| PathBuf::from(String::from(s)));
    let file_save_root = env
        .get_string(&file_save_dir)
        .ok()
        .map(|s| PathBuf::from(String::from(s)));

    let port = if port == 0 {
        crate::protocol::DEFAULT_PORT
    } else {
        port as u16
    };

    let config = EngineConfig {
        device_name: name,
        port,
        ..EngineConfig::default()
    };
    let config = config_with_android_paths(config, data_root, file_save_root);

    let (tx, rx) = mpsc::channel(256);
    match rt().block_on(Engine::start(config, tx)) {
        Ok(engine) => {
            let handle = Box::new(AndroidHandle {
                engine,
                event_rx: rx,
            });
            Box::into_raw(handle) as jlong
        }
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", format!("{:#}", e));
            0
        }
    }
}

fn config_with_android_paths(
    config: EngineConfig,
    data_root: Option<PathBuf>,
    file_save_root: Option<PathBuf>,
) -> EngineConfig {
    let mut updated = config;

    if let Some(data_root) = data_root.filter(|path| !path.as_os_str().is_empty()) {
        updated.trust_store_path = data_root.join("trust.json");
        updated.peer_store_path = data_root.join("peers.json");
        updated.identity_path = data_root.join("identity.key");
    }

    if let Some(file_save_root) = file_save_root.filter(|path| !path.as_os_str().is_empty()) {
        updated.file_save_dir = Some(file_save_root);
    }

    updated
}

// ── stop ──────────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_stop(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle != 0 {
        unsafe { drop(Box::from_raw(handle as *mut AndroidHandle)) };
    }
}

// ── pushText ──────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_pushText(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    text: JString,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let s: String = match env.get_string(&text) {
        Ok(s) => s.into(),
        Err(_) => return -1,
    };
    let h = unsafe { &*(handle as *const AndroidHandle) };
    rt().block_on(h.engine.push_clipboard(ClipboardContent::Text(s))) as jint
}

// ── pushImage ─────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_pushImage(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    mime: JString,
    data: jbyteArray,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let mime: String = env.get_string(&mime).map(|s| s.into()).unwrap_or_default();
    let data = unsafe { JByteArray::from_raw(data) };
    let bytes = match env.convert_byte_array(&data) {
        Ok(b) => b,
        Err(_) => return -1,
    };
    let h = unsafe { &*(handle as *const AndroidHandle) };
    rt().block_on(
        h.engine
            .push_clipboard(ClipboardContent::Image { mime, data: bytes }),
    ) as jint
}

// ── pushFile ──────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_pushFile(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    name: JString,
    data: jbyteArray,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let name: String = env.get_string(&name).map(|s| s.into()).unwrap_or_default();
    let data = unsafe { JByteArray::from_raw(data) };
    let bytes = match env.convert_byte_array(&data) {
        Ok(b) => b,
        Err(_) => return -1,
    };
    let h = unsafe { &*(handle as *const AndroidHandle) };
    rt().block_on(
        h.engine
            .push_clipboard(ClipboardContent::File { name, data: bytes }),
    ) as jint
}

// ── pollEvent ─────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_pollEvent(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jlong {
    if handle == 0 {
        return 0;
    }
    let h = unsafe { &mut *(handle as *mut AndroidHandle) };
    match h.event_rx.try_recv() {
        Ok(event) => Box::into_raw(Box::new(event)) as jlong,
        Err(_) => 0,
    }
}

// ── Notifications ─────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_pushNotification(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    id: JString,
    package_name: JString,
    title: JString,
    text: JString,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let ctx = unsafe { &*(handle as *const AndroidHandle) };

    let id = env.get_string(&id).map(|s| s.into()).unwrap_or_default();
    let package = env
        .get_string(&package_name)
        .map(|s| s.into())
        .unwrap_or_default();
    let title = env.get_string(&title).map(|s| s.into()).unwrap_or_default();
    let text = env.get_string(&text).map(|s| s.into()).unwrap_or_default();

    rt().block_on(async {
        ctx.engine.push_notification(id, package, title, text).await;
    });
    0
}

// ── Event polling ─────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventType(
    _env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jint {
    if event == 0 {
        return 0;
    }
    use crate::engine::EngineEvent::*;
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    match ev {
        ClipboardReceived {
            content,
            auto_applied,
            ..
        } => match content {
            ClipboardContent::Text(_) => {
                if *auto_applied {
                    1
                } else {
                    11
                }
            } // 11 = available but not applied
            ClipboardContent::Image { .. } => 2,
            ClipboardContent::File { .. } => 3,
        },
        HistoryMetadataReceived { .. } => 7,
        ClipboardSynced { .. } => 8,
        ClipboardSyncFailed { .. } => 7,
        TofuPrompt { .. } => 4,
        PeerConnected { .. } => 5,
        PeerDisconnected { .. } => 6,
        FileTransferIncoming { .. } => 12,
        FileTransferProgress { .. } => 13,
        FileTransferComplete { .. } => 14,
        FileTransferFailed { .. } => 15,
        FileTransferPaused { .. } => 20,
        FileTransferResumed { .. } => 21,
        ActivityFeedUpdated { .. } => 16,
        CallStateChanged { .. } => 17,
        CallActionRequest { .. } => 18,
        BatteryStateChanged { .. } => 19,
        NotificationReceived { .. } => 16,
        Warning(_) => 7,
    }
}

// ── eventText ─────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventText(
    mut env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    if let crate::engine::EngineEvent::ClipboardReceived {
        content: ClipboardContent::Text(ref t),
        ..
    } = ev
    {
        env.new_string(t)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut())
    } else {
        std::ptr::null_mut()
    }
}

// ── eventBinaryData ───────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventBinaryData(
    env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jbyteArray {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    let bytes = match ev {
        crate::engine::EngineEvent::ClipboardReceived {
            content: ClipboardContent::Image { data, .. },
            ..
        } => Some(data.as_slice()),
        crate::engine::EngineEvent::ClipboardReceived {
            content: ClipboardContent::File { data, .. },
            ..
        } => Some(data.as_slice()),
        _ => None,
    };

    bytes
        .and_then(|data| env.byte_array_from_slice(data).ok())
        .map(|array| array.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

// ── eventDeviceName ───────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventDeviceName(
    mut env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    use crate::engine::EngineEvent::*;
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    let name: Option<&str> = match ev {
        ClipboardReceived { from_name, .. } => Some(from_name.as_str()),
        HistoryMetadataReceived { from_name, .. } => Some(from_name.as_str()),
        ClipboardSynced { peer_name, .. } => Some(peer_name.as_str()),
        ClipboardSyncFailed { peer_name, .. } => Some(peer_name.as_str()),
        TofuPrompt { device_name, .. } => Some(device_name.as_str()),
        PeerConnected { device_name, .. } => Some(device_name.as_str()),
        PeerDisconnected { device_name, .. } => device_name.as_deref(),
        _ => None,
    };
    name.and_then(|n| env.new_string(n).ok())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

// ── eventDeviceId ─────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventDeviceId(
    mut env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    use crate::engine::EngineEvent::*;
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    let id = match ev {
        ClipboardReceived { from_device, .. } => Some(*from_device),
        HistoryMetadataReceived { from_device, .. } => Some(*from_device),
        ClipboardSynced { peer_device, .. } => Some(*peer_device),
        ClipboardSyncFailed { peer_device, .. } => Some(*peer_device),
        TofuPrompt { device_id, .. } => Some(*device_id),
        PeerConnected { device_id, .. } => Some(*device_id),
        PeerDisconnected { device_id, .. } => Some(*device_id),
        FileTransferIncoming { from_device, .. } => Some(*from_device),
        FileTransferProgress { from_device, .. } => Some(*from_device),
        FileTransferComplete { from_device, .. } => Some(*from_device),
        FileTransferFailed { from_device, .. } => Some(*from_device),
        _ => None,
    };
    id.and_then(|value| env.new_string(value.to_string()).ok())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

// ── eventMimeType ─────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventMimeType(
    mut env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    if let crate::engine::EngineEvent::ClipboardReceived {
        content: ClipboardContent::Image { mime, .. },
        ..
    } = ev
    {
        env.new_string(mime)
            .ok()
            .map(|value| value.into_raw())
            .unwrap_or(std::ptr::null_mut())
    } else {
        std::ptr::null_mut()
    }
}

// ── eventFileName ─────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventFileName(
    mut env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    if let crate::engine::EngineEvent::ClipboardReceived {
        content: ClipboardContent::File { name, .. },
        ..
    } = ev
    {
        env.new_string(name)
            .ok()
            .map(|value| value.into_raw())
            .unwrap_or(std::ptr::null_mut())
    } else {
        std::ptr::null_mut()
    }
}

// ── eventFingerprint ──────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventFingerprint(
    mut env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    if let crate::engine::EngineEvent::TofuPrompt {
        fingerprint_display,
        ..
    } = ev
    {
        env.new_string(fingerprint_display)
            .ok()
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut())
    } else {
        std::ptr::null_mut()
    }
}

// ── eventAutoApplied ─────────────────────────────────────────────────────────
/// Returns 1 if this ClipboardReceived event was auto-applied to the local
/// clipboard, or 0 if it was only recorded in the activity feed (timeline-first).

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventAutoApplied(
    _env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jint {
    if event == 0 {
        return 0;
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    if let crate::engine::EngineEvent::ClipboardReceived { auto_applied, .. } = ev {
        if *auto_applied {
            1
        } else {
            0
        }
    } else {
        0
    }
}

// ── eventActivityId ──────────────────────────────────────────────────────────
/// Returns the activity feed entry ID for a ClipboardReceived event.
/// The Kotlin layer uses this to show the "Apply" button in the timeline.

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventActivityId(
    _env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jlong {
    if event == 0 {
        return -1;
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    if let crate::engine::EngineEvent::ClipboardReceived { activity_id, .. } = ev {
        *activity_id as jlong
    } else {
        -1
    }
}

// ── eventTransferId ──────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventTransferId(
    mut env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    let tid = match ev {
        crate::engine::EngineEvent::FileTransferIncoming { transfer_id, .. } => {
            Some(hex::encode(transfer_id))
        }
        crate::engine::EngineEvent::FileTransferProgress { transfer_id, .. } => {
            Some(hex::encode(transfer_id))
        }
        crate::engine::EngineEvent::FileTransferComplete { transfer_id, .. } => {
            Some(hex::encode(transfer_id))
        }
        crate::engine::EngineEvent::FileTransferFailed { transfer_id, .. } => {
            Some(hex::encode(transfer_id))
        }
        crate::engine::EngineEvent::FileTransferPaused { transfer_id, .. } => {
            Some(hex::encode(transfer_id))
        }
        crate::engine::EngineEvent::FileTransferResumed { transfer_id, .. } => {
            Some(hex::encode(transfer_id))
        }
        _ => None,
    };
    tid.and_then(|s| env.new_string(s).ok())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

// ── eventTransferFileName ────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventTransferFileName(
    mut env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    let name = match ev {
        crate::engine::EngineEvent::FileTransferIncoming { file_name, .. } => {
            Some(file_name.as_str())
        }
        crate::engine::EngineEvent::FileTransferProgress { file_name, .. } => {
            Some(file_name.as_str())
        }
        crate::engine::EngineEvent::FileTransferComplete { file_name, .. } => {
            Some(file_name.as_str())
        }
        _ => None,
    };
    name.and_then(|s| env.new_string(s).ok())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

// ── eventTransferProgressPercent ─────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventTransferProgressPercent(
    _env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jint {
    if event == 0 {
        return -1;
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    if let crate::engine::EngineEvent::FileTransferProgress { percent, .. } = ev {
        *percent as jint
    } else {
        -1
    }
}

// ── eventTransferBytesReceived ───────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventTransferBytesReceived(
    _env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jlong {
    if event == 0 {
        return -1;
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    if let crate::engine::EngineEvent::FileTransferProgress {
        bytes_received, ..
    } = ev
    {
        *bytes_received as jlong
    } else {
        -1
    }
}

// ── eventTransferSpeedBps ────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventTransferSpeedBps(
    _env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jlong {
    if event == 0 {
        return -1;
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    if let crate::engine::EngineEvent::FileTransferProgress { speed_bps, .. } = ev {
        speed_bps.unwrap_or(0) as jlong
    } else {
        -1
    }
}

// ── eventTransferEtaSecs ─────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventTransferEtaSecs(
    _env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jlong {
    if event == 0 {
        return -1;
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    if let crate::engine::EngineEvent::FileTransferProgress { eta_secs, .. } = ev {
        eta_secs.map(|value| value as jlong).unwrap_or(-1)
    } else {
        -1
    }
}

// ── eventTransferTotalBytes ──────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventTransferTotalBytes(
    _env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jlong {
    if event == 0 {
        return -1;
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    match ev {
        crate::engine::EngineEvent::FileTransferIncoming { file_bytes, .. } => *file_bytes as jlong,
        crate::engine::EngineEvent::FileTransferProgress { total_bytes, .. } => {
            *total_bytes as jlong
        }
        _ => -1,
    }
}

// ── eventTransferDestPath ────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventTransferDestPath(
    mut env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    if let crate::engine::EngineEvent::FileTransferComplete { dest_path, .. } = ev {
        env.new_string(dest_path.to_string_lossy())
            .ok()
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut())
    } else {
        std::ptr::null_mut()
    }
}

// ── applyClipboardByHash ─────────────────────────────────────────────────────
/// Called from Kotlin when the user taps "Apply" on a timeline entry.

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_applyClipboardByHash(
    mut env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    hash_jstr: JString,
) -> jint {
    if engine_ptr == 0 {
        return 0;
    }
    let hash: String = {
        let s = match env.get_string(&hash_jstr) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        s.into()
    };
    let h = unsafe { &*(engine_ptr as *const AndroidHandle) };
    match rt().block_on(h.engine.apply_clipboard_by_hash(hash)) {
        Ok(true) => 1,
        _ => 0,
    }
}

// ── trustPeer / rejectPeer ───────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_trustPeer(
    mut env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    device_id_jstr: JString,
) -> jint {
    if engine_ptr == 0 {
        return 0;
    }
    let device_id: String = match env.get_string(&device_id_jstr) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };
    let Ok(device_id) = uuid::Uuid::parse_str(&device_id) else {
        return 0;
    };
    let h = unsafe { &*(engine_ptr as *const AndroidHandle) };
    match rt().block_on(h.engine.trust_peer(device_id)) {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_rejectPeer(
    mut env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    device_id_jstr: JString,
) -> jint {
    if engine_ptr == 0 {
        return 0;
    }
    let device_id: String = match env.get_string(&device_id_jstr) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };
    let Ok(device_id) = uuid::Uuid::parse_str(&device_id) else {
        return 0;
    };
    let h = unsafe { &*(engine_ptr as *const AndroidHandle) };
    match rt().block_on(h.engine.reject_peer(device_id)) {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

// ── acceptFileTransfer ───────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_acceptFileTransfer(
    mut env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    transfer_id_hex: JString,
) -> jint {
    if engine_ptr == 0 {
        return 0;
    }
    let hex_str: String = {
        let s = match env.get_string(&transfer_id_hex) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        s.into()
    };
    let Ok(bytes) = hex::decode(&hex_str) else {
        return 0;
    };
    let Ok(tid): Result<[u8; 16], _> = bytes.try_into() else {
        return 0;
    };
    let h = unsafe { &*(engine_ptr as *const AndroidHandle) };
    match rt().block_on(h.engine.accept_file_transfer(tid)) {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

// ── rejectFileTransfer ───────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_rejectFileTransfer(
    mut env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    transfer_id_hex: JString,
) -> jint {
    if engine_ptr == 0 {
        return 0;
    }
    let hex_str: String = {
        let s = match env.get_string(&transfer_id_hex) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        s.into()
    };
    let Ok(bytes) = hex::decode(&hex_str) else {
        return 0;
    };
    let Ok(tid): Result<[u8; 16], _> = bytes.try_into() else {
        return 0;
    };
    let h = unsafe { &*(engine_ptr as *const AndroidHandle) };
    match rt().block_on(h.engine.reject_file_transfer(tid, "user rejected".into())) {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_cancelFileTransfer(
    mut env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    transfer_id_hex: JString,
) -> jint {
    if engine_ptr == 0 {
        return 0;
    }
    let hex_str: String = {
        let s = match env.get_string(&transfer_id_hex) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        s.into()
    };
    let Ok(bytes) = hex::decode(&hex_str) else {
        return 0;
    };
    let Ok(tid): Result<[u8; 16], _> = bytes.try_into() else {
        return 0;
    };
    let h = unsafe { &*(engine_ptr as *const AndroidHandle) };
    match rt().block_on(h.engine.cancel_file_transfer(tid)) {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_pauseFileTransfer(
    mut env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    transfer_id_hex: JString,
) -> jint {
    if engine_ptr == 0 {
        return 0;
    }
    let hex_str: String = {
        let s = match env.get_string(&transfer_id_hex) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        s.into()
    };
    let Ok(bytes) = hex::decode(&hex_str) else {
        return 0;
    };
    let Ok(tid): Result<[u8; 16], _> = bytes.try_into() else {
        return 0;
    };
    let h = unsafe { &*(engine_ptr as *const AndroidHandle) };
    match rt().block_on(h.engine.pause_file_transfer(tid)) {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_resumeFileTransfer(
    mut env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    transfer_id_hex: JString,
) -> jint {
    if engine_ptr == 0 {
        return 0;
    }
    let hex_str: String = {
        let s = match env.get_string(&transfer_id_hex) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        s.into()
    };
    let Ok(bytes) = hex::decode(&hex_str) else {
        return 0;
    };
    let Ok(tid): Result<[u8; 16], _> = bytes.try_into() else {
        return 0;
    };
    let h = unsafe { &*(engine_ptr as *const AndroidHandle) };
    match rt().block_on(h.engine.resume_file_transfer(tid)) {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

// ── connectToPeer ───────────────────────────────────────────────────────────────
/// Called from Kotlin when Android NSD resolves a Deskdrop peer on the LAN.
/// Returns 0 on success, -1 on error.

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_connectToPeer(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    ip: JString,
    port: jint,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let ip_str: String = match env.get_string(&ip) {
        Ok(s) => s.into(),
        Err(_) => return -1,
    };
    let h = unsafe { &*(handle as *const AndroidHandle) };
    match rt().block_on(h.engine.connect_to_peer(ip_str, port as u16)) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_disconnectPeer(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    device_id_jstr: JString,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let device_id: String = match env.get_string(&device_id_jstr) {
        Ok(s) => s.into(),
        Err(_) => return -1,
    };
    let Ok(device_id) = uuid::Uuid::parse_str(&device_id) else {
        return -1;
    };
    let h = unsafe { &*(handle as *const AndroidHandle) };
    match rt().block_on(h.engine.disconnect_peer(device_id)) {
        Ok(true) => 1,
        Ok(false) => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_sendFilePath(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    path: JString,
    display_name: JString,
    mime_type: JString,
    target_device_id: JString,
) -> jint {
    if handle == 0 {
        return -1;
    }

    let path: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(_) => return -1,
    };
    let display_name: String = match env.get_string(&display_name) {
        Ok(s) => s.into(),
        Err(_) => return -1,
    };
    let mime_type: String = match env.get_string(&mime_type) {
        Ok(s) => s.into(),
        Err(_) => return -1,
    };
    let target_device = if target_device_id.is_null() {
        None
    } else {
        let raw: String = match env.get_string(&target_device_id) {
            Ok(s) => s.into(),
            Err(_) => return -1,
        };
        match uuid::Uuid::parse_str(&raw) {
            Ok(value) => Some(value),
            Err(_) => return -1,
        }
    };

    let h = unsafe { &*(handle as *const AndroidHandle) };
    match rt().block_on(h.engine.send_file_path(
        PathBuf::from(path),
        display_name,
        mime_type,
        target_device,
    )) {
        Ok(_) => 1,
        Err(_) => -1,
    }
}

// ── freeEvent ─────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_freeEvent(
    _env: JNIEnv,
    _class: JClass,
    event: jlong,
) {
    if event != 0 {
        unsafe { drop(Box::from_raw(event as *mut crate::engine::EngineEvent)) };
    }
}

// ── applySyncSettings ─────────────────────────────────────────────────────────
/// Atomically update the engine's sync-filter flags without restarting.
/// Called when the user toggles sync options in SettingsActivity.
///
/// Returns 0 on success, -1 if the handle is invalid.
#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_applySyncSettings(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    sync_enabled: jboolean,
    sync_text: jboolean,
    sync_images: jboolean,
    sync_files: jboolean,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let h = unsafe { &*(handle as *const AndroidHandle) };
    rt().block_on(h.engine.apply_sync_settings(
        sync_enabled != 0,
        sync_text != 0,
        sync_images != 0,
        sync_files != 0,
    ));
    0
}
/// Returns the engine's stable device UUID as a hyphenated lowercase string,
/// e.g. "550e8400-e29b-41d4-a716-446655440000".
///
/// Kotlin uses this to filter out self-connections during NSD resolution:
/// the mDNS service name is "deskdrop-<first-8-chars-of-uuid>" so we can
/// skip resolved peers whose service name prefix matches our own UUID prefix.
///
/// Returns null (0) if the handle is invalid.
#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_getDeviceId(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jstring {
    if handle == 0 {
        return std::ptr::null_mut();
    }
    let h = unsafe { &*(handle as *const AndroidHandle) };
    let uuid_str = rt().block_on(h.engine.device_id()).to_string();
    match env.new_string(&uuid_str) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_peersJson(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jstring {
    if handle == 0 {
        return std::ptr::null_mut();
    }
    let h = unsafe { &*(handle as *const AndroidHandle) };
    let peers = rt().block_on(h.engine.status_snapshot()).peers;
    let json = serde_json::to_string(&peers).unwrap_or_else(|_| "[]".to_string());
    match env.new_string(json) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

// ── Call continuity JNI exports ───────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_pushCallState(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    state: JString,
    number: JString,
    contact_name: JString,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let state: String = match env.get_string(&state) {
        Ok(s) => s.into(),
        Err(_) => return -1,
    };
    let number: String = env.get_string(&number).map(|s| s.into()).unwrap_or_default();
    let contact_name: String = env
        .get_string(&contact_name)
        .map(|s| s.into())
        .unwrap_or_default();
    let h = unsafe { &*(handle as *const AndroidHandle) };
    rt().block_on(h.engine.push_call_state(state, number, contact_name));
    0
}

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventCallState(
    mut env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    let val = match ev {
        crate::engine::EngineEvent::CallStateChanged { state, .. } => Some(state.as_str()),
        _ => None,
    };
    val.and_then(|s| env.new_string(s).ok())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventCallNumber(
    mut env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    let val = match ev {
        crate::engine::EngineEvent::CallStateChanged { number, .. } => Some(number.as_str()),
        _ => None,
    };
    val.and_then(|s| env.new_string(s).ok())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventCallContactName(
    mut env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    let val = match ev {
        crate::engine::EngineEvent::CallStateChanged {
            contact_name, ..
        } => Some(contact_name.as_str()),
        _ => None,
    };
    val.and_then(|s| env.new_string(s).ok())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_eventCallAction(
    mut env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    let val = match ev {
        crate::engine::EngineEvent::CallActionRequest { action, .. } => Some(action.as_str()),
        _ => None,
    };
    val.and_then(|s| env.new_string(s).ok())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_com_cliprelay_ClipRelayJni_pushBatteryStatus(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    level: jint,
    charging: jboolean,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let h = unsafe { &*(handle as *const AndroidHandle) };
    rt().block_on(h.engine.push_battery_status(level as u8, charging != 0));
    0
}
