use axum::{
    http::{header, StatusCode, Uri},
    response::{Html, IntoResponse, Response},
};

const INDEX_HTML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/index.html"));

pub async fn serve_index() -> impl IntoResponse {
    Html(INDEX_HTML)
}

pub async fn serve_static(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches("/static/");
    let content_type = match std::path::Path::new(path).extension() {
        Some(ext) => match ext.to_str() {
            Some("css") => "text/css",
            Some("js") => "application/javascript",
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("svg") => "image/svg+xml",
            Some("woff2") => "font/woff2",
            _ => "application/octet-stream",
        },
        None => "application/octet-stream",
    };

    // Only index.html is embedded; other static files not supported in this minimal build
    let body = match path {
        "index.html" => INDEX_HTML,
        _ => {
            return (StatusCode::NOT_FOUND, "Not found").into_response();
        }
    };

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, content_type)],
        body,
    )
        .into_response()
}
