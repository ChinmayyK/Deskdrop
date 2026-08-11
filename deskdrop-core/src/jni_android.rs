//! JNI bridge for Android.
//!
//! Exposes the same Deskdrop engine to Kotlin/Java via JNI.
//! This file lives alongside the other Rust sources and is compiled
//! into libdeskdrop_core.so for each Android ABI.
//!
//! Generated JNI signatures match the Kotlin declarations in
//! DeskdropJni.kt (package com.deskdrop, object DeskdropJni).

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
    event_tx: mpsc::Sender<crate::engine::EngineEvent>,
}

// ── start ─────────────────────────────────────────────────────────────────────

static ANDROID_CONTEXT: OnceLock<jni::objects::GlobalRef> = OnceLock::new();

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_initContext(
    env: JNIEnv,
    _class: JClass,
    context: jni::objects::JObject,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if ANDROID_CONTEXT.get().is_some() {
            return;
        }
        if context.is_null() {
            return;
        }
        let vm = match env.get_java_vm() {
            Ok(vm) => vm,
            Err(_) => return,
        };
        let ctx_ref = match env.new_global_ref(context) {
            Ok(ref_) => ref_,
            Err(_) => return,
        };
        let vm_ptr = vm.get_java_vm_pointer() as *mut std::ffi::c_void;
        let ctx_ptr = ctx_ref.as_obj().as_raw() as *mut std::ffi::c_void;

        if ANDROID_CONTEXT.set(ctx_ref).is_ok() {
            let _ = std::panic::catch_unwind(|| unsafe {
                ndk_context::initialize_android_context(vm_ptr, ctx_ptr);
            });
        }
    }));
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_start(
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
    let event_tx = tx.clone();
    match rt().block_on(Engine::start(config, tx)) {
        Ok(engine) => {
            let handle = Box::new(AndroidHandle {
                engine,
                event_rx: rx,
                event_tx,
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_stop(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_pushText(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_pushImage(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    mime: JString,
    data: jbyteArray,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let data = unsafe { JByteArray::from_raw(data) };
    let size = env.get_array_length(&data).unwrap_or(0) as usize;
    if size > crate::protocol::MAX_IMAGE_BYTES {
        tracing::warn!("Ignoring clipboard image because it exceeds 32MB limit");
        return -1;
    }
    let mime: String = env.get_string(&mime).map(|s| s.into()).unwrap_or_default();
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_pushFile(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    name: JString,
    data: jbyteArray,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let data = unsafe { JByteArray::from_raw(data) };
    let size = env.get_array_length(&data).unwrap_or(0) as usize;
    if size > crate::protocol::MAX_IMAGE_BYTES {
        // Use same 32MB limit
        tracing::warn!("Ignoring clipboard file because it exceeds 32MB limit");
        return -1;
    }
    let name: String = env.get_string(&name).map(|s| s.into()).unwrap_or_default();
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
// ── pushVideoFrame ──────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_pushVideoFrame(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    data: jbyteArray,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let data = unsafe { JByteArray::from_raw(data) };
    let bytes = match env.convert_byte_array(&data) {
        Ok(b) => b,
        Err(_) => return -1,
    };
    let h = unsafe { &*(handle as *const AndroidHandle) };
    rt().block_on(h.engine.push_camera_frame(bytes));
    0
}

// ── stopCameraStream ──────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_stopCameraStream(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let h = unsafe { &*(handle as *const AndroidHandle) };
    rt().block_on(h.engine.stop_camera_stream());
    0
}

// ── pollEvent ─────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_pollEvent(
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

// ── waitEvent ─────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_waitEvent(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jlong {
    if handle == 0 {
        return 0;
    }
    let h = unsafe { &mut *(handle as *mut AndroidHandle) };
    let fut = h.event_rx.recv();
    match rt().block_on(fut) {
        Some(event) => Box::into_raw(Box::new(event)) as jlong,
        None => 0,
    }
}

// ── interruptWait ─────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_interruptWait(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle == 0 {
        return;
    }
    let h = unsafe { &*(handle as *const AndroidHandle) };
    let _ = h
        .event_tx
        .try_send(crate::engine::EngineEvent::Warning("interrupt".into()));
}

// ── Notifications ─────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_pushNotification(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventType(
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
        } => match &**content {
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
        PairingRequested { .. } => 4,
        OutgoingPairingWaiting { .. } => 29,
        SystemHealthUpdated(_) => 26,
        ClipboardDeliveryStatus { .. } => 7,
        PairingConfirmed { .. } => 7,
        PairingRejected { .. } => 7,
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
        NetworkStateChanged { .. } => 28,
        NotificationReceived { .. } => 16,
        CameraStreamRequest { .. } => 22,
        CameraStreamAccept { .. } => 23,
        CameraStreamStop { .. } => 24,
        CameraFrameReceived { .. } => 25,
        PairingRequest { .. } => 7,
        PairingResponse { .. } => 7,
        PeerDiscovered { .. } => 5,
        RemoteFilesQueryReceived { .. } => 30,
        RemoteThumbnailRequestReceived { .. } => 31,
        RemoteFilePullRequestReceived { .. } => 32,
        RemoteFileActionRequestReceived { .. } => 37,
        RemoteFilesResponseReceived { .. } => 33,
        RemoteThumbnailResponseReceived { .. } => 34,
        SpeedTestProgress { .. } => 35,
        SpeedTestComplete { .. } => 36,
        Warning(_) => 7,
        PeerSyncStateChanged { .. } => 16,
    }
}

// ── eventText ─────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventText(
    env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    match ev {
        crate::engine::EngineEvent::ClipboardReceived { content, .. } => {
            if let ClipboardContent::Text(ref t) = &**content {
                return env
                    .new_string(t)
                    .map(|s| s.into_raw())
                    .unwrap_or(std::ptr::null_mut());
            }
        }
        crate::engine::EngineEvent::Warning(msg) => {
            return env
                .new_string(msg)
                .map(|s| s.into_raw())
                .unwrap_or(std::ptr::null_mut());
        }
        crate::engine::EngineEvent::PairingResponse { accepted, .. } => {
            let msg = if *accepted {
                "Pairing request was accepted."
            } else {
                "Pairing request was declined."
            };
            return env
                .new_string(msg)
                .map(|s| s.into_raw())
                .unwrap_or(std::ptr::null_mut());
        }
        crate::engine::EngineEvent::RemoteFileActionRequestReceived { action, .. } => {
            return env
                .new_string(action)
                .map(|s| s.into_raw())
                .unwrap_or(std::ptr::null_mut());
        }
        _ => {}
    }
    std::ptr::null_mut()
}

// ── eventBinaryData ───────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventBinaryData(
    env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jbyteArray {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    let bytes = match ev {
        crate::engine::EngineEvent::ClipboardReceived { content, .. } => match &**content {
            ClipboardContent::Image { data, .. } => Some(data.as_slice()),
            ClipboardContent::File { data, .. } => Some(data.as_slice()),
            _ => None,
        },
        _ => None,
    };

    bytes
        .and_then(|data| env.byte_array_from_slice(data).ok())
        .map(|array| array.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

// ── eventDeviceName ───────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventDeviceName(
    env: JNIEnv,
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
        PairingRequested { device_name, .. } => Some(device_name.as_str()),
        OutgoingPairingWaiting { device_name, .. } => Some(device_name.as_str()),
        ClipboardDeliveryStatus { .. } => None,
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventDeviceId(
    env: JNIEnv,
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
        PairingRequested { device_id, .. } => Some(*device_id),
        OutgoingPairingWaiting { device_id, .. } => Some(*device_id),
        PairingConfirmed { device_id, .. } => Some(*device_id),
        PairingRejected { device_id, .. } => Some(*device_id),
        ClipboardDeliveryStatus { .. } => None,
        PeerConnected { device_id, .. } => Some(*device_id),
        PeerDisconnected { device_id, .. } => Some(*device_id),
        FileTransferIncoming { from_device, .. } => Some(*from_device),
        FileTransferProgress { from_device, .. } => Some(*from_device),
        FileTransferComplete { from_device, .. } => Some(*from_device),
        FileTransferFailed { from_device, .. } => Some(*from_device),
        RemoteFilesQueryReceived { from_device, .. } => Some(*from_device),
        RemoteThumbnailRequestReceived { from_device, .. } => Some(*from_device),
        RemoteFilePullRequestReceived { from_device, .. } => Some(*from_device),
        RemoteFileActionRequestReceived { from_device, .. } => Some(*from_device),
        RemoteFilesResponseReceived { from_device, .. } => Some(*from_device),
        RemoteThumbnailResponseReceived { from_device, .. } => Some(*from_device),
        _ => None,
    };
    id.and_then(|value| env.new_string(value.to_string()).ok())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

// ── eventMimeType ─────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventMimeType(
    env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    if let crate::engine::EngineEvent::ClipboardReceived { content, .. } = ev {
        if let ClipboardContent::Image { mime, .. } = &**content {
            return env
                .new_string(mime)
                .ok()
                .map(|value| value.into_raw())
                .unwrap_or(std::ptr::null_mut());
        }
    }
    std::ptr::null_mut()
}

// ── eventFileName ─────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventFileName(
    env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    if let crate::engine::EngineEvent::ClipboardReceived { content, .. } = ev {
        if let ClipboardContent::File { name, .. } = &**content {
            return env
                .new_string(name)
                .ok()
                .map(|value| value.into_raw())
                .unwrap_or(std::ptr::null_mut());
        }
    }
    std::ptr::null_mut()
}

// ── eventFingerprint ──────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventFingerprint(
    env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    match ev {
        crate::engine::EngineEvent::PairingRequested { pin, .. } => env
            .new_string(pin)
            .ok()
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        crate::engine::EngineEvent::OutgoingPairingWaiting { pin, .. } => env
            .new_string(pin)
            .ok()
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        _ => std::ptr::null_mut(),
    }
}

// ── eventAutoApplied ─────────────────────────────────────────────────────────
/// Returns 1 if this ClipboardReceived event was auto-applied to the local
/// clipboard, or 0 if it was only recorded in the activity feed (timeline-first).

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventAutoApplied(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventActivityId(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventTransferId(
    env: JNIEnv,
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventTransferFileName(
    env: JNIEnv,
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventTransferProgressPercent(
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

// ── eventSpeedTest ───────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventSpeedTestBytes(
    _env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jlong {
    if event == 0 {
        return -1;
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    if let crate::engine::EngineEvent::SpeedTestProgress {
        bytes_transferred, ..
    } = ev
    {
        *bytes_transferred as jlong
    } else {
        -1
    }
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventSpeedTestDuration(
    _env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jint {
    if event == 0 {
        return -1;
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    if let crate::engine::EngineEvent::SpeedTestProgress { duration_secs, .. } = ev {
        *duration_secs as jint
    } else {
        -1
    }
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventSpeedTestPhase(
    env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    let phase_str = match ev {
        crate::engine::EngineEvent::SpeedTestProgress { direction, .. } => direction.as_str(),
        _ => return std::ptr::null_mut(),
    };
    env.new_string(phase_str)
        .unwrap_or_else(|_| env.new_string("").unwrap())
        .into_raw()
}

// ── eventTransferBytesReceived ───────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventTransferBytesReceived(
    _env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jlong {
    if event == 0 {
        return -1;
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    if let crate::engine::EngineEvent::FileTransferProgress { bytes_received, .. } = ev {
        *bytes_received as jlong
    } else {
        -1
    }
}

// ── eventTransferSpeedBps ────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventTransferSpeedBps(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventTransferEtaSecs(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventTransferTotalBytes(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventTransferDestPath(
    env: JNIEnv,
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_applyClipboardByHash(
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

// ── startSpeedTest ───────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_startSpeedTest(
    mut env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    device_id_jstr: JString,
    duration_secs: jint,
) -> jint {
    if engine_ptr == 0 {
        return 0;
    }
    let device_id_str: String = {
        match env.get_string(&device_id_jstr) {
            Ok(s) => s.into(),
            Err(_) => return 0,
        }
    };
    let target_uuid = match uuid::Uuid::parse_str(&device_id_str) {
        Ok(u) => u,
        Err(_) => return 0,
    };

    let h = unsafe { &*(engine_ptr as *const AndroidHandle) };
    match rt().block_on(h.engine.start_speed_test(target_uuid, duration_secs as u32)) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

// ── sendPermissionError ──────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_sendPermissionError<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    engine_ptr: jlong,
    device_id_jstr: JString<'local>,
    feature_jstr: JString<'local>,
    message_jstr: JString<'local>,
) -> jint {
    if engine_ptr == 0 {
        return 0;
    }
    let device_id_str: String = match env.get_string(&device_id_jstr) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };
    let feature_str: String = match env.get_string(&feature_jstr) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };
    let message_str: String = match env.get_string(&message_jstr) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };
    let target_uuid = match uuid::Uuid::parse_str(&device_id_str) {
        Ok(u) => u,
        Err(_) => return 0,
    };

    let h = unsafe { &*(engine_ptr as *const AndroidHandle) };

    // We send a generic AppMessage::PermissionError
    let msg = crate::protocol::AppMessage::PermissionError {
        feature: feature_str,
        message: message_str,
        origin_device: h.engine.local_device_id(),
        origin_device_name: h.engine.local_device_name(),
    };

    match rt().block_on(h.engine.send_message(target_uuid, msg)) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

// ── trustPeer / rejectPeer ───────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_trustPeer(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_trustPeerFromQr(
    mut env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    device_id_jstr: JString,
    token_jstr: JString,
) -> jint {
    if engine_ptr == 0 {
        return 0;
    }
    let device_id: String = match env.get_string(&device_id_jstr) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };
    let token: String = match env.get_string(&token_jstr) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };
    let Ok(device_id) = uuid::Uuid::parse_str(&device_id) else {
        return 0;
    };
    let h = unsafe { &*(engine_ptr as *const AndroidHandle) };
    match rt().block_on(h.engine.trust_peer(device_id)) {
        Ok(()) => {
            rt().block_on(h.engine.send_qr_auth(device_id, token));
            1
        }
        Err(_) => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_rejectPeer(
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

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_forgetPeer(
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
    match rt().block_on(h.engine.forget_device(device_id)) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_sendPairingRequest(
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
    rt().block_on(h.engine.send_pairing_request(device_id));
    1
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_respondToPairing(
    mut env: JNIEnv,
    _class: JClass,
    engine_ptr: jlong,
    device_id_jstr: JString,
    accepted: jboolean,
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
    let is_accepted = accepted != 0;
    match rt().block_on(h.engine.respond_to_pairing(device_id, is_accepted)) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

// ── acceptFileTransfer ───────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_acceptFileTransfer(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_rejectFileTransfer(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_cancelFileTransfer(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_pauseFileTransfer(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_resumeFileTransfer(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_connectToPeer(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_reportDiscoveredPeer(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    device_id_jstr: JString,
    device_name_jstr: JString,
    ip: JString,
    port: jint,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let device_id_str: String = match env.get_string(&device_id_jstr) {
        Ok(s) => s.into(),
        Err(_) => return -1,
    };
    let Ok(device_id) = uuid::Uuid::parse_str(&device_id_str) else {
        return -1;
    };
    let device_name_str: String = match env.get_string(&device_name_jstr) {
        Ok(s) => s.into(),
        Err(_) => return -1,
    };
    let ip_str: String = match env.get_string(&ip) {
        Ok(s) => s.into(),
        Err(_) => return -1,
    };
    let h = unsafe { &*(handle as *const AndroidHandle) };
    match rt().block_on(h.engine.report_discovered_peer(
        device_id,
        device_name_str,
        ip_str,
        port as u16,
    )) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_initiatePairing(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    device_id_jstr: JString,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let device_id_str: String = match env.get_string(&device_id_jstr) {
        Ok(s) => s.into(),
        Err(_) => return -1,
    };
    let Ok(device_id) = uuid::Uuid::parse_str(&device_id_str) else {
        return -1;
    };
    let h = unsafe { &*(handle as *const AndroidHandle) };
    match rt().block_on(h.engine.initiate_pairing(device_id)) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_reconnectPeer(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    device_id_jstr: JString,
) -> jboolean {
    if handle == 0 {
        return 0;
    }
    let device_id: String = match env.get_string(&device_id_jstr) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };
    let Ok(device_id) = uuid::Uuid::parse_str(&device_id) else {
        return 0;
    };
    let h = unsafe { &*(handle as *const AndroidHandle) };
    match rt().block_on(h.engine.reconnect_peer_by_id(device_id)) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_disconnectPeer(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_setSyncEnabled(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    enabled: jboolean,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let h = unsafe { &*(handle as *const AndroidHandle) };
    let b = enabled != 0;
    match rt().block_on(h.engine.set_sync_enabled(b)) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_sendFilePath(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    path: JString,
    display_name: JString,
    mime_type: JString,
    target_device_id: JString,
    batch_id: JString,
    is_directory: jboolean,
    item_count: jint,
) -> jstring {
    if handle == 0 {
        return std::ptr::null_mut();
    }

    let path: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let display_name: String = match env.get_string(&display_name) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let mime_type: String = match env.get_string(&mime_type) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };
    let target_device = if target_device_id.is_null() {
        None
    } else {
        let raw: String = match env.get_string(&target_device_id) {
            Ok(s) => s.into(),
            Err(_) => return std::ptr::null_mut(),
        };
        match uuid::Uuid::parse_str(&raw) {
            Ok(value) => Some(value),
            Err(_) => return std::ptr::null_mut(),
        }
    };
    let batch_id = if batch_id.is_null() {
        None
    } else {
        match env.get_string(&batch_id) {
            Ok(s) => Some(s.into()),
            Err(_) => None,
        }
    };

    let h = unsafe { &*(handle as *const AndroidHandle) };
    match rt().block_on(h.engine.send_file_path(
        PathBuf::from(path),
        display_name,
        mime_type,
        target_device,
        batch_id,
        is_directory != 0,
        item_count as u32,
    )) {
        Ok(tid) => env
            .new_string(hex::encode(tid))
            .ok()
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        Err(_) => std::ptr::null_mut(),
    }
}

// ── freeEvent ─────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_freeEvent(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_applySyncSettings(
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_getDeviceId(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jstring {
    if handle == 0 {
        return std::ptr::null_mut();
    }
    let h = unsafe { &*(handle as *const AndroidHandle) };
    let uuid_str = h.engine.device_id().to_string();
    match env.new_string(&uuid_str) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_peersJson(
    env: JNIEnv,
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_pushCallState(
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
    let number: String = env
        .get_string(&number)
        .map(|s| s.into())
        .unwrap_or_default();
    let contact_name: String = env
        .get_string(&contact_name)
        .map(|s| s.into())
        .unwrap_or_default();
    let h = unsafe { &*(handle as *const AndroidHandle) };
    rt().block_on(h.engine.push_call_state(state, number, contact_name));
    0
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventCallState(
    env: JNIEnv,
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventCallNumber(
    env: JNIEnv,
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventCallContactName(
    env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    let val = match ev {
        crate::engine::EngineEvent::CallStateChanged { contact_name, .. } => {
            Some(contact_name.as_str())
        }
        _ => None,
    };
    val.and_then(|s| env.new_string(s).ok())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventCallAction(
    env: JNIEnv,
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
pub extern "system" fn Java_com_deskdrop_DeskdropJni_pushBatteryStatus(
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

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_pushNetworkStatus(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    network_type: JString,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let ntype = env
        .get_string(&network_type)
        .map(|s| s.into())
        .unwrap_or_else(|_| "offline".to_string());
    let h = unsafe { &*(handle as *const AndroidHandle) };
    rt().block_on(h.engine.push_network_status(ntype));
    0
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_pushStorageStatus(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    images_bytes: jlong,
    videos_bytes: jlong,
    apps_bytes: jlong,
    free_bytes: jlong,
    total_bytes: jlong,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let h = unsafe { &*(handle as *const AndroidHandle) };
    rt().block_on(h.engine.push_storage_status(
        images_bytes as u64,
        videos_bytes as u64,
        apps_bytes as u64,
        free_bytes as u64,
        total_bytes as u64,
    ));
    0
}

// ── notifyNetworkRestored ────────────────────────────────────────────────────
/// Called from Kotlin when Android's ConnectivityManager reports that the
/// default network has become available again (e.g., after Doze, Wi-Fi
/// reconnect, or airplane mode toggle). Triggers an immediate reconnection
/// attempt to all known trusted peers.

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_notifyNetworkRestored(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let h = unsafe { &*(handle as *const AndroidHandle) };
    let engine = h.engine.clone();
    rt().spawn(async move {
        engine.reconnect_all_peers().await;
    });
    0
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_notifySleepState(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    is_asleep: jboolean,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let h = unsafe { &*(handle as *const AndroidHandle) };
    let engine = h.engine.clone();
    rt().spawn(async move {
        engine.notify_sleep_state(is_asleep != 0).await;
    });
    0
}

// ── Remote Explorer JNI Accessors & Response Senders ──────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventRequestId(
    env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    use crate::engine::EngineEvent::*;
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    let req_id = match ev {
        RemoteFilesQueryReceived { request_id, .. } => Some(request_id.to_string()),
        RemoteFilesResponseReceived { request_id, .. } => Some(request_id.to_string()),
        RemoteThumbnailRequestReceived { request_id, .. } => Some(request_id.to_string()),
        RemoteThumbnailResponseReceived { request_id, .. } => Some(request_id.to_string()),
        RemoteFilePullRequestReceived { request_id, .. } => Some(request_id.to_string()),
        _ => None,
    };
    req_id
        .and_then(|s| env.new_string(s).ok())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventSummaryOnly(
    _env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jboolean {
    if event == 0 {
        return 0;
    }
    use crate::engine::EngineEvent::*;
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    match ev {
        RemoteFilesQueryReceived { summary_only, .. } => {
            if *summary_only {
                1
            } else {
                0
            }
        }
        _ => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventFileId(
    _env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jlong {
    if event == 0 {
        return 0;
    }
    use crate::engine::EngineEvent::*;
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    match ev {
        RemoteThumbnailRequestReceived { file_id, .. } => *file_id as jlong,
        RemoteThumbnailResponseReceived { file_id, .. } => *file_id as jlong,
        RemoteFilePullRequestReceived { file_id, .. } => *file_id as jlong,
        RemoteFileActionRequestReceived { file_id, .. } => *file_id as jlong,
        _ => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventThumbnailSizePx(
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
        RemoteThumbnailRequestReceived { size_px, .. } => *size_px as jint,
        _ => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventOffset(
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
        RemoteFilesQueryReceived { offset, .. } => *offset as jint,
        _ => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventLimit(
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
        RemoteFilesQueryReceived { limit, .. } => *limit as jint,
        _ => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventFileCategory(
    env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    use crate::engine::EngineEvent::*;
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    let cat = match ev {
        RemoteFilesQueryReceived { category, .. } => match category {
            Some(crate::protocol::RemoteFileCategory::Images) => Some("Images"),
            Some(crate::protocol::RemoteFileCategory::Videos) => Some("Videos"),
            Some(crate::protocol::RemoteFileCategory::Audio) => Some("Audio"),
            Some(crate::protocol::RemoteFileCategory::Documents) => Some("Documents"),
            Some(crate::protocol::RemoteFileCategory::Apks) => Some("Apks"),
            Some(crate::protocol::RemoteFileCategory::Archives) => Some("Archives"),
            _ => Some("All"),
        },
        _ => None,
    };
    cat.and_then(|s| env.new_string(s).ok())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventFileSource(
    env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    use crate::engine::EngineEvent::*;
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    let src = match ev {
        RemoteFilesQueryReceived { source, .. } => match source {
            Some(crate::protocol::RemoteFileSource::WhatsApp) => Some("WhatsApp"),
            Some(crate::protocol::RemoteFileSource::Downloads) => Some("Downloads"),
            Some(crate::protocol::RemoteFileSource::Camera) => Some("Camera"),
            _ => Some("All"),
        },
        _ => None,
    };
    src.and_then(|s| env.new_string(s).ok())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_eventSearchQuery(
    env: JNIEnv,
    _class: JClass,
    event: jlong,
) -> jstring {
    if event == 0 {
        return std::ptr::null_mut();
    }
    use crate::engine::EngineEvent::*;
    let ev = unsafe { &*(event as *const crate::engine::EngineEvent) };
    let q = match ev {
        RemoteFilesQueryReceived { search_query, .. } => search_query.as_deref(),
        _ => None,
    };
    q.and_then(|s| env.new_string(s).ok())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_sendRemoteFilesResponse(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    request_id: JString,
    target_device_id: JString,
    summary_json: JString,
    files_json: JString,
    total_matching: jint,
    error: JString,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let req_raw: String = match env.get_string(&request_id) {
        Ok(s) => s.into(),
        Err(_) => return -1,
    };
    let req_id = match uuid::Uuid::parse_str(&req_raw) {
        Ok(u) => u,
        Err(_) => return -1,
    };
    let tgt_raw: String = match env.get_string(&target_device_id) {
        Ok(s) => s.into(),
        Err(_) => return -1,
    };
    let target_device = match uuid::Uuid::parse_str(&tgt_raw) {
        Ok(u) => u,
        Err(_) => return -1,
    };

    let summary: Option<crate::protocol::RemoteFilesSummary> = if summary_json.is_null() {
        None
    } else {
        match env.get_string(&summary_json) {
            Ok(s) => serde_json::from_str(&String::from(s)).ok(),
            Err(_) => None,
        }
    };

    let files: Vec<crate::protocol::RemoteFileEntry> = if files_json.is_null() {
        Vec::new()
    } else {
        match env.get_string(&files_json) {
            Ok(s) => serde_json::from_str(&String::from(s)).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    };

    let err_str = if error.is_null() {
        None
    } else {
        env.get_string(&error).ok().map(|s| s.into())
    };

    let h = unsafe { &*(handle as *const AndroidHandle) };
    rt().block_on(h.engine.send_remote_files_response(
        target_device,
        req_id,
        summary,
        files,
        total_matching as u32,
        err_str,
    ));
    0
}

#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_sendRemoteThumbnailResponse(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    request_id: JString,
    target_device_id: JString,
    file_id: jlong,
    data: jbyteArray,
    error: JString,
) -> jint {
    if handle == 0 {
        return -1;
    }
    let req_raw: String = match env.get_string(&request_id) {
        Ok(s) => s.into(),
        Err(_) => return -1,
    };
    let req_id = match uuid::Uuid::parse_str(&req_raw) {
        Ok(u) => u,
        Err(_) => return -1,
    };
    let tgt_raw: String = match env.get_string(&target_device_id) {
        Ok(s) => s.into(),
        Err(_) => return -1,
    };
    let target_device = match uuid::Uuid::parse_str(&tgt_raw) {
        Ok(u) => u,
        Err(_) => return -1,
    };

    let bytes = if data.is_null() {
        Vec::new()
    } else {
        let jarr = unsafe { JByteArray::from_raw(data) };
        env.convert_byte_array(&jarr).unwrap_or_default()
    };

    let err_str = if error.is_null() {
        None
    } else {
        env.get_string(&error).ok().map(|s| s.into())
    };

    let h = unsafe { &*(handle as *const AndroidHandle) };
    rt().block_on(h.engine.send_remote_thumbnail_response(
        target_device,
        req_id,
        file_id as u64,
        bytes,
        err_str,
    ));
    0
}
