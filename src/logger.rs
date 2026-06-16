use std::path::PathBuf;

use axum::{
    extract::{ws::Message, ws::WebSocket, State, WebSocketUpgrade},
    response::IntoResponse,
};
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt};
use tokio::time::{interval, Duration};
use tracing::warn;

use crate::models::AppState;
use crate::AppError;

pub async fn websocket_logs(
    State(state): State<std::sync::Arc<AppState>>,
    axum::extract::Path(program_id): axum::extract::Path<String>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, AppError> {
    let log_dir = state.process_manager.read().await.log_dir(&program_id);
    Ok(ws.on_upgrade(move |socket| handle_log_socket(socket, log_dir)))
}

async fn handle_log_socket(mut socket: WebSocket, log_dir: PathBuf) {
    let stdout_path = log_dir.join("stdout.log");
    let mut last_size: u64 = 0;
    let mut ticker = interval(Duration::from_secs(1));

    loop {
        ticker.tick().await;

        if !stdout_path.exists() {
            if socket.send(Message::Text("[Log file not yet created]\n".to_string())).await.is_err() {
                break;
            }
            continue;
        }

        let metadata = match fs::metadata(&stdout_path).await {
            Ok(m) => m,
            Err(_) => continue,
        };
        let current_size = metadata.len();

        if current_size < last_size {
            last_size = 0;
        }

        if current_size > last_size {
            let file = match fs::File::open(&stdout_path).await {
                Ok(f) => f,
                Err(_) => continue,
            };
            let mut reader = tokio::io::BufReader::new(file);
            if let Err(e) = tokio::io::AsyncSeekExt::seek(&mut reader, std::io::SeekFrom::Start(last_size)).await {
                warn!("Seek error: {}", e);
                continue;
            }
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let msg = format!("{}\n", line);
                if socket.send(Message::Text(msg)).await.is_err() {
                    return;
                }
            }
            last_size = current_size;
        }
    }
}

pub async fn search_logs(
    State(state): State<std::sync::Arc<AppState>>,
    axum::extract::Path(program_id): axum::extract::Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<axum::Json<serde_json::Value>, AppError> {
    let keyword = params.get("keyword").cloned().unwrap_or_default();
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(500);

    let log_dir = state.process_manager.read().await.log_dir(&program_id);
    let stdout_path = log_dir.join("stdout.log");
    let stderr_path = log_dir.join("stderr.log");

    let mut results = Vec::new();

    for path in [&stdout_path, &stderr_path] {
        if !path.exists() {
            continue;
        }
        let file = fs::File::open(path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to open log: {}", e)))?;
        let reader = tokio::io::BufReader::new(file);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if keyword.is_empty() || line.contains(&keyword) {
                results.push(line);
                if results.len() >= limit {
                    break;
                }
            }
        }
        if results.len() >= limit {
            break;
        }
    }

    Ok(axum::Json(serde_json::json!({ "lines": results })))
}
