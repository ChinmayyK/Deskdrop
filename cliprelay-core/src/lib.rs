//! # cliprelay-core
//!
//! Proximity-Based Shared Clipboard — Rust engine library.
//!
//! ## Architecture
//!
//! ```text
//!  ┌───────────────────────────────────────────────────────────────────┐
//!  │                        Platform Layer                              │
//!  │  macOS (Swift)   Windows (C#)   Android (Kotlin/JNI)  Linux (GTK) │
//!  └────────────────────────────┬──────────────────────────────────────┘
//!                               │  C FFI / JNI / Unix IPC
//!  ┌────────────────────────────▼──────────────────────────────────────┐
//!  │                       cliprelay-core                              │
//!  │                                                                    │
//!  │  engine.rs ──► network.rs ──► crypto.rs                           │
//!  │      │               │                                             │
//!  │      │           chunked.rs  (streaming large payloads)           │
//!  │      │               │                                             │
//!  │  discovery.rs     dedup.rs   (echo suppression, rate limiting)    │
//!  │  (mDNS-SD)        pairing.rs (PIN-based secure pairing)           │
//!  │      │                                                             │
//!  │  trust.rs       settings.rs    history.rs    metrics.rs           │
//!  │  (TOFU/PIN)     (user config)  (ring buffer)  (stats)             │
//!  │                                                                    │
//!  │  ipc.rs    (Unix socket / named pipe daemon <-> CLI bridge)        │
//!  │  protocol.rs  (wire format)                                        │
//!  └────────────────────────────────────────────────────────────────────┘
//! ```

pub mod chunked;
pub mod crypto;
pub mod dedup;
pub mod discovery;
pub mod engine;
pub mod ffi;
pub mod history;
pub mod ipc;
pub mod metrics;
pub mod network;
pub mod network_manager;
pub mod pairing;
pub mod peer_manager;
pub mod protocol;
pub mod settings;
pub mod trust;

#[cfg(target_os = "android")]
pub mod jni_android;

pub use engine::{Engine, EngineConfig, EngineEvent};
pub use protocol::ClipboardContent;
pub use settings::{Settings, SettingsStore};
pub mod compress;
pub mod filter;
pub mod identity;
#[cfg(windows)]
pub mod ipc_windows;
pub mod probe;
pub mod retry;
pub mod sim;
pub mod sync_controller;
pub mod throttle;
