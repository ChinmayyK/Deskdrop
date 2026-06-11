use axum::{
    extract::{Multipart, State},
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use axum::http::StatusCode;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use uuid::Uuid;
use crate::engine::EngineEvent;

#[derive(Clone)]
pub struct WebDashboardState {
    pub device_name: String,
    pub save_dir: PathBuf,
    pub event_tx: mpsc::Sender<EngineEvent>,
    pub web_device_id: Uuid, // A dummy UUID to represent the "Web Guest"
}

pub async fn start_web_server(
    port: u16,
    state: WebDashboardState,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/", get(serve_ui))
        .route("/api/info", get(api_info))
        .route("/api/upload", post(api_upload))
        .with_state(Arc::new(state));

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Web Dashboard running on http://{}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.recv().await;
            tracing::info!("Web Dashboard stopping...");
        })
        .await?;

    Ok(())
}

async fn serve_ui() -> Html<&'static str> {
    Html(include_str!("dashboard_ui.html"))
}

async fn api_info(State(state): State<Arc<WebDashboardState>>) -> impl IntoResponse {
    Json(json!({ "device_name": state.device_name }))
}

async fn api_upload(
    State(state): State<Arc<WebDashboardState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut total_uploaded = 0;
    
    while let Ok(Some(mut field)) = multipart.next_field().await {
        let file_name = if let Some(file_name) = field.file_name() {
            file_name.to_owned()
        } else {
            continue;
        };

        // Attempt to extract content length from headers if present
        let mut total_bytes = 0; // Unknown by default in multipart unless specified

        let dest_path = state.save_dir.join(&file_name);
        let mut file = match tokio::fs::File::create(&dest_path).await {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("Failed to create file: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create file");
            }
        };

        let transfer_id = *uuid::Uuid::new_v4().as_bytes();
        let from_device = state.web_device_id;
        let mut bytes_received = 0u64;

        while let Ok(Some(chunk)) = field.chunk().await {
            if let Err(e) = file.write_all(&chunk).await {
                tracing::error!("Failed to write chunk: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Write error");
            }
            bytes_received += chunk.len() as u64;

            // Optional: debounce this so we don't spam events.
            let _ = state.event_tx.try_send(EngineEvent::FileTransferProgress {
                transfer_id,
                from_device,
                file_name: file_name.clone(),
                percent: if total_bytes > 0 { ((bytes_received as f64 / total_bytes as f64) * 100.0) as u8 } else { 0 },
                bytes_received,
                total_bytes,
                speed_bps: None,
                eta_secs: None,
            });
        }

        total_uploaded += 1;

        let _ = state.event_tx.send(EngineEvent::FileTransferComplete {
            transfer_id,
            from_device,
            from_name: "Web Guest".to_string(),
            file_name,
            dest_path,
        }).await;
    }

    if total_uploaded > 0 {
        (StatusCode::OK, "Upload complete")
    } else {
        (StatusCode::BAD_REQUEST, "No files uploaded")
    }
}
