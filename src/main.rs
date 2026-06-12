// =====================================================================
// Project: Zero-Trust API Gateway
// Author: Phung The Vinh (ptvstar2003@gmail.com)
// Copyright © 2026. All rights reserved.
// =====================================================================

use axum::{Router, routing::get};
use config::Config;
use std::sync::Arc;

mod config;
mod proxy;

use proxy::{AppState, proxy_handler};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    match Config::load("config.yaml") {
        Ok(config) => {
            println!("Đã tải cấu hình: {:#?}", config);
            println!("--------------------------------------------------");
            println!(
                "- Máy chủ hoạt động tại: {}:{}",
                config.server.host, config.server.port
            );

            println!("- Mức log: {}", config.server.log_level);
            println!("- Redis: {}", config.database.redis_url);
            println!("- JWT Secret: {}", config.security.jwt.secret_key_path);
            println!(
                "- Zero Trust Private Key: {}",
                config.security.zero_trust.private_key_path
            );
            println!("- AI Native Model Path: {}", config.ai_native.model_path);
            println!(
                "- AI Native Similarity Threshold: {}",
                config.ai_native.similarity_threshold
            );
            println!("- AI Native Cache TTL: {}", config.ai_native.cache_ttl);
            println!("- Routes: {:#?}", config.routes);
            println!("--------------------------------------------------");
            // 1. Khởi tạo AppState dùng chung
            let state = AppState {
                config: Arc::new(config.clone()),
                client: reqwest::Client::new(),
            };
            // 2. Dựng Router và gắn State
            let app = Router::new()
                .route("/health", get(|| async { "OK" }))
                .fallback(proxy_handler)
                .with_state(state);
            // 3. Lắng nghe và khởi chạy Server
            let addr = format!("{}:{}", config.server.host, config.server.port);
            let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

            // Log thông báo server bắt đầu lắng nghe
            tracing::info!("API Gateway đang chạy trên http://{}", addr);
            axum::serve(listener, app).await.unwrap();
        }
        Err(e) => {
            eprintln!("Lỗi khi tải cấu hình: {}", e);
        }
    }
}
