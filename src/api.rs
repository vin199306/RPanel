use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde_json::json;

use crate::auth::login;
use crate::logger::{search_logs, websocket_logs};
use crate::models::{
    AppState, Program, ProgramRequest, ProgramResponse, ProgramUpdateRequest,
};
use crate::monitor::{get_process_stats, get_system_info};
use crate::AppError;

pub fn public_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/login", post(login))
        .with_state(state)
}

pub fn protected_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/programs", get(list_programs).post(create_program))
        .route(
            "/api/programs/{id}",
            get(get_program).put(update_program).delete(delete_program),
        )
        .route("/api/programs/{id}/start", post(start_program))
        .route("/api/programs/{id}/stop", post(stop_program))
        .route("/api/programs/{id}/restart", post(restart_program))
        .route("/api/system/info", get(system_info))
        .route("/api/system/monitor", get(system_monitor))
        .route("/api/programs/{id}/stats", get(program_stats))
        .route("/api/programs/{id}/logs", get(search_logs))
        .route("/api/programs/{id}/logs/ws", get(websocket_logs))
        .with_state(state)
}

async fn list_programs(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let mut pm = state.process_manager.write().await;
    pm.refresh_status().await;
    let programs: Vec<ProgramResponse> = pm.list().iter().map(|p| (*p).into()).collect();
    Ok((StatusCode::OK, Json(json!({ "programs": programs }))))
}

async fn create_program(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ProgramRequest>,
) -> Result<impl IntoResponse, AppError> {
    if req.name.trim().is_empty() || req.command.trim().is_empty() {
        return Err(AppError::BadRequest("Name and command are required".to_string()));
    }
    let mut program = Program::new(req.name, req.command, req.work_dir);
    if let Some(args) = req.args {
        program.args = args;
    }
    if let Some(env) = req.env {
        program.env = env;
    }
    if let Some(auto_start) = req.auto_start {
        program.auto_start = auto_start;
    }
    let mut pm = state.process_manager.write().await;
    let id = pm.create(program).await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

async fn get_program(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let mut pm = state.process_manager.write().await;
    pm.refresh_status().await;
    let program = pm.get(&id).ok_or(AppError::NotFound)?;
    let resp: ProgramResponse = program.into();
    Ok((StatusCode::OK, Json(resp)))
}

async fn update_program(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<ProgramUpdateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let mut pm = state.process_manager.write().await;
    pm.update(&id, req).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_program(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let mut pm = state.process_manager.write().await;
    pm.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn start_program(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let mut pm = state.process_manager.write().await;
    pm.start(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn stop_program(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let mut pm = state.process_manager.write().await;
    pm.stop(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn restart_program(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let mut pm = state.process_manager.write().await;
    pm.restart(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn system_info() -> Result<impl IntoResponse, AppError> {
    let info = get_system_info().map_err(|e| AppError::Internal(e.to_string()))?;
    Ok((StatusCode::OK, Json(info)))
}

async fn system_monitor() -> Result<impl IntoResponse, AppError> {
    let info = get_system_info().map_err(|e| AppError::Internal(e.to_string()))?;
    Ok((StatusCode::OK, Json(info)))
}

async fn program_stats(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let mut pm = state.process_manager.write().await;
    pm.refresh_status().await;
    let program = pm.get(&id).ok_or(AppError::NotFound)?;
    let pid = program.pid.ok_or(AppError::BadRequest("Program is not running".to_string()))?;
    let stats = get_process_stats(pid).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok((StatusCode::OK, Json(stats)))
}
