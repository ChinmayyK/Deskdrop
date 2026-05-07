//! JNI bridge for Android.
//!
//! Exposes the same ClipRelay engine to Kotlin/Java via JNI.
//! This file lives alongside the other Rust sources and is compiled
//! into libcliprelay_core.so for each Android ABI.
//!
//! Generated JNI signatures match the Kotlin declarations in
//! ClipRelayJni.kt (package com.cliprelay, object ClipRelayJni).

#![cfg(target_os = "android")]

use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jbyteArray, jint, jlong, jstring};
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
pub extern "system" fn Java_com_proxiboard_ClipRelayJni_start(
    mut env: JNIEnv,
    _class: JClass,
    device_name: JString,
    port: jint,
    data_dir: JString,
) -> jlong {
    let name: String = env
        .get_string(&device_name)
        .map(|s| s.into())
        .unwrap_or_else(|_| whoami::devicename());
    let data_root = env
        .get_string(&data_dir)
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
    let config = config_with_android_paths(config, data_root);

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

fn config_with_android_paths(config: EngineConfig, data_root: Option<PathBuf>) -> EngineConfig {
    let Some(data_root) = data_root.filter(|path| !path.as_os_str().is_empty()) else {
        return config;
    };

    EngineConfig {
        trust_store_path: data_root.join("trust.json"),
        peer_store_path: data_root.join("peers.json"),
        identity_path: data_root.join("identity.key"),
        ..config
    }
}

// ── stop ──────────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_proxiboard_ClipRelayJni_stop(
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
pub extern "system" fn Java_com_proxiboard_ClipRelayJni_pushText(
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
pub extern "system" fn Java_com_proxiboard_ClipRelayJni_pushImage(
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
pub extern "system" fn Java_com_proxiboard_ClipRelayJni_pushFile(
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
pub extern "system" fn Java_com_proxiboard_ClipRelayJni_pollEvent(
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

// ── eventType ─────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_proxiboard_ClipRelayJni_eventType(
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
        ClipboardReceived { content, .. } => match content {
            ClipboardContent::Text(_) => 1,
            ClipboardContent::Image { .. } => 2,
            ClipboardContent::File { .. } => 3,
        },
        HistoryMetadataReceived { .. } => 7,
        ClipboardSynced { .. } => 8,
        ClipboardSyncFailed { .. } => 7,
        TofuPrompt { .. } => 4,
        PeerConnected { .. } => 5,
        PeerDisconnected { .. } => 6,
        Warning(_) => 7,
    }
}

// ── eventText ─────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_proxiboard_ClipRelayJni_eventText(
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
pub extern "system" fn Java_com_proxiboard_ClipRelayJni_eventBinaryData(
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
pub extern "system" fn Java_com_proxiboard_ClipRelayJni_eventDeviceName(
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

// ── eventMimeType ─────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_proxiboard_ClipRelayJni_eventMimeType(
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
pub extern "system" fn Java_com_proxiboard_ClipRelayJni_eventFileName(
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
pub extern "system" fn Java_com_proxiboard_ClipRelayJni_eventFingerprint(
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

// ── freeEvent ─────────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_com_proxiboard_ClipRelayJni_freeEvent(
    _env: JNIEnv,
    _class: JClass,
    event: jlong,
) {
    if event != 0 {
        unsafe { drop(Box::from_raw(event as *mut crate::engine::EngineEvent)) };
    }
}
