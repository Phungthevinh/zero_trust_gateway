use axum::{
    extract::Path,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

pub async fn dashboard_handler(path: Option<Path<String>>) -> Response {
    let raw_path = match &path {
        Some(Path(p)) => p.as_str(),
        None => "",
    };
    let clean_path = raw_path.trim_start_matches('/');
    let file_path = if clean_path.is_empty() {
        "index.html"
    } else {
        clean_path
    };

    match Assets::get(file_path) {
        Some(file) => {
            let mime_type = match file_path.rsplit('.').next() {
                Some("html") => "text/html",
                Some("css") => "text/css",
                Some("js") => "application/javascript",
                Some("json") => "application/json",
                Some("svg") => "image/svg+xml",
                _ => "application/octet-stream",
            };

            (
                [(header::CONTENT_TYPE, HeaderValue::from_static(mime_type))],
                file.data,
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
}
