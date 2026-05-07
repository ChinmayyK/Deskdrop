//! ClipRelay Linux — headless daemon + optional GTK4 tray.
//! Build with --features gtk for the full system-tray experience.
//! Without it, runs as a headless background daemon.

use cliprelay_core::{
    engine::{Engine, EngineConfig, EngineEvent},
    protocol::ClipboardContent,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("CLIPRELAY_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let rt = Arc::new(tokio::runtime::Runtime::new().expect("Tokio runtime"));
    let (event_tx, event_rx) = mpsc::channel(256);
    let config = EngineConfig::default();

    let engine = rt
        .block_on(Engine::start(config, event_tx))
        .expect("ClipRelay engine failed to start");

    tracing::info!(
        "ClipRelay Linux. IPC socket: {:?}",
        cliprelay_core::ipc::socket_path()
    );

    // Clipboard watcher (arboard).
    let engine_clip = Arc::new(engine);
    let engine_for_clip = engine_clip.clone();
    let rt_clip = rt.clone();

    std::thread::spawn(move || {
        let mut cb = match arboard::Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("clipboard init: {}", e);
                return;
            }
        };
        let mut last = String::new();
        loop {
            std::thread::sleep(Duration::from_millis(100));
            if let Ok(t) = cb.get_text() {
                if t != last && !t.is_empty() {
                    last = t.clone();
                    rt_clip.block_on(engine_for_clip.push_clipboard(ClipboardContent::Text(t)));
                }
            }
        }
    });

    // Event drain.
    rt.block_on(async move {
        let mut rx = event_rx;
        loop {
            tokio::select! {
                Some(event) = rx.recv() => handle_event(event),
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Shutting down.");
                    break;
                }
            }
        }
    });
}

fn handle_event(event: EngineEvent) {
    match event {
        EngineEvent::ClipboardReceived {
            from_name, content, ..
        } => {
            tracing::info!("📋 {} from '{}'", content.kind_str(), from_name);
            if let ClipboardContent::Text(ref t) = content {
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(t);
                }
            }
            notify(&format!("📋 Clipboard from {}", from_name));
        }
        EngineEvent::HistoryMetadataReceived { from_name, .. } => {
            tracing::info!("history metadata received from '{}'", from_name)
        }
        EngineEvent::ClipboardSynced { peer_name, .. } => {
            notify(&format!("✅ Clipboard synced with {}", peer_name))
        }
        EngineEvent::ClipboardSyncFailed {
            peer_name, reason, ..
        } => tracing::warn!("sync to '{}' failed: {}", peer_name, reason),
        EngineEvent::PeerConnected { device_name, .. } => {
            notify(&format!("📡 Connected to {}", device_name))
        }
        EngineEvent::TofuPrompt {
            device_name,
            fingerprint_display,
            ..
        } => tracing::warn!(
            "🔐 New device '{}'\n   FP: {}",
            device_name,
            fingerprint_display
        ),
        EngineEvent::PeerDisconnected { .. } => {}
        EngineEvent::Warning(w) => tracing::warn!("{}", w),
    }
}

fn notify(body: &str) {
    let _ = std::process::Command::new("notify-send")
        .args([
            "--app-name=ClipRelay",
            "--icon=edit-paste",
            "ClipRelay",
            body,
        ])
        .spawn();
}
