use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{middleware, Router};
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

mod api;
mod auth;
mod config;
mod frontend;
mod logger;
mod models;
mod monitor;
mod process_mgr;
mod protect;

use crate::auth::auth_middleware;
use crate::config::AppConfig;
use crate::models::AppState;
use crate::process_mgr::ProcessManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("minipanel=info,tower_http=info")
        .init();

    let data_dir = PathBuf::from(
        std::env::var("MINIPANEL_DATA_DIR").unwrap_or_else(|_| "/opt/minipanel/data".to_string()),
    );
    let config_path = data_dir.join("config.json");
    let cert_path = data_dir.join("server.crt");
    let key_path = data_dir.join("server.key");

    tokio::fs::create_dir_all(&data_dir).await?;
    tokio::fs::create_dir_all(data_dir.join("logs")).await?;

    let config = AppConfig::load_or_default(&config_path).await?;

    let process_manager = Arc::new(RwLock::new(ProcessManager::new(data_dir.clone())));
    {
        let mut pm = process_manager.write().await;
        pm.load_programs().await?;
        pm.auto_start().await;
    }

    let state = Arc::new(AppState {
        data_dir: data_dir.clone(),
        config: Arc::new(RwLock::new(config)),
        config_path,
        process_manager: process_manager.clone(),
    });

    let app = Router::new()
        .route("/", get(frontend::serve_index))
        .route("/static/{*path}", get(frontend::serve_static))
        .merge(api::public_routes(state.clone()))
        .merge(
            api::protected_routes(state.clone())
                .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware)),
        )
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .with_state(state.clone());

    let addr: SocketAddr = ([0, 0, 0, 0], 8080).into();

    #[cfg(feature = "tls")]
    if cert_path.exists() && key_path.exists() {
        use std::fs::File;
        use std::io::BufReader as StdBufReader;
        use std::sync::Arc;
        use tokio::net::TcpListener;
        use tokio_rustls::TlsAcceptor;

        let certs: Vec<_> = rustls_pemfile::certs(&mut StdBufReader::new(File::open(&cert_path)?))
            .filter_map(|c| c.ok())
            .collect();
        let key = rustls_pemfile::private_key(&mut StdBufReader::new(File::open(&key_path)?))
            .map_err(|e| anyhow::anyhow!("Failed to read private key: {}", e))?
            .ok_or_else(|| anyhow::anyhow!("No private key found"))?;
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| anyhow::anyhow!("TLS config error: {}", e))?;
        let acceptor = TlsAcceptor::from(Arc::new(config));

        let listener = TcpListener::bind(addr).await?;
        info!("Starting HTTPS server on {}", addr);

        loop {
            let (stream, _) = listener.accept().await?;
            let acceptor = acceptor.clone();
            let service = app.clone();
            tokio::spawn(async move {
                let Ok(stream) = acceptor.accept(stream).await else { return };
                let io = hyper_util::rt::TokioIo::new(stream);
                let service = hyper::service::service_fn(move |req| {
                    let service = service.clone();
                    async move {
                        Ok::<_, std::convert::Infallible>(service.oneshot(req).await)
                    }
                });
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(io, service)
                    .await;
            });
        }
    }

    warn!("No TLS certificate found, falling back to HTTP");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, msg) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "Not Found".to_string()),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            AppError::Forbidden(m) => (StatusCode::FORBIDDEN, m),
            AppError::Internal(m) => {
                tracing::error!("Internal error: {}", m);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string())
            }
        };
        (status, axum::Json(serde_json::json!({ "error": msg }))).into_response()
    }
}
