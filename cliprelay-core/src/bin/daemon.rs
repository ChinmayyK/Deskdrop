use cliprelay_core::engine::{Engine, EngineConfig, EngineEvent};
use std::sync::Arc;
use tokio::sync::mpsc;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("CLIPRELAY_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let rt = Arc::new(tokio::runtime::Runtime::new().expect("Tokio runtime"));
    let (event_tx, mut event_rx) = mpsc::channel(256);
    let config = EngineConfig::default();

    let _engine = rt
        .block_on(Engine::start(config, event_tx))
        .expect("ClipRelay engine failed to start");

    tracing::info!(
        "ClipRelay Daemon started. IPC socket: {:?}",
        cliprelay_core::ipc::socket_path()
    );

    // Event drain.
    rt.block_on(async move {
        loop {
            tokio::select! {
                Some(event) = event_rx.recv() => handle_event(event),
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
        }
        EngineEvent::PeerConnected { device_name, .. } => {
            tracing::info!("📡 Connected to {}", device_name);
        }
        EngineEvent::PeerDisconnected { device_name, .. } => {
            tracing::info!("🔌 Disconnected from {:?}", device_name);
        }
        EngineEvent::Warning(w) => tracing::warn!("{}", w),
        _ => {}
    }
}
