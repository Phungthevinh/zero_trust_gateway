use crate::config::Config;
use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    response::IntoResponse,
};
use std::sync::Arc;

// Định nghĩa AppState chia sẻ dữ liệu giữa các luồng
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub client: reqwest::Client,
}

// Handler nhận request và khớp route
pub async fn proxy_handler(State(state): State<AppState>, req: Request<Body>) -> impl IntoResponse {
    let path = req.uri().path();

    // Duyệt qua danh sách routes để tìm route khớp
    let matched_route = state
        .config
        .routes
        .iter()
        .find(|r| path.starts_with(&r.path));

    match matched_route {
        Some(route) => {
            tracing::info!(
                "Khớp route cấu hình: {} -> Target: {}",
                route.path,
                route.target
            );
            // Tạm thời trả về text để kiểm tra định tuyến
            format!("Đã khớp route: Target Upstream là {}", route.target).into_response()
        }
        None => {
            tracing::warn!("Không tìm thấy route khớp cho path: {}", path);
            (
                StatusCode::NOT_FOUND,
                "Không tìm thấy đường dẫn cấu hình tại Gateway",
            )
                .into_response()
        }
    }
}
