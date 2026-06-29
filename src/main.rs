// =====================================================================
// Project: Zero-Trust API Gateway
// Author: Phung The Vinh (ptvstar2003@gmail.com)
// Copyright © 2026. All rights reserved.
// =====================================================================

use axum::{Router, routing::get};
use config::Config;
use jsonwebtoken::DecodingKey;
use std::sync::Arc;

mod auth;
mod config;
mod fast_reject;
mod proxy;
mod rate_limit;
mod redis_rate_limit;
mod signature;

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

            //khởi tạo public key
            let public_key_pem = match std::fs::read(&config.security.jwt.secret_key_path) {
                Ok(bytes) => bytes,
                Err(e) => {
                    tracing::error!("Lỗi khi đọc public key: {}", e);
                    return;
                }
            };

            //khởi tạo decoding key
            let decoding_key = DecodingKey::from_rsa_pem(&public_key_pem).unwrap();

            // Load Ed25519 private key cho Zero-Trust signing
            let signing_key =
                match signature::load_private_key(&config.security.zero_trust.private_key_path)
                    .await
                {
                    Ok(key) => Arc::new(key),
                    Err(e) => {
                        tracing::error!("Lỗi khi đọc private key: {}", e);
                        return;
                    }
                };

            // Khởi tạo Rate Limiter cục bộ (ví dụ: tối đa 100 request/giây, hồi phục 10 req/s, TTL 60 giây)
            let rate_limiter = Arc::new(rate_limit::RateLimiter::new(100.0, 10.0, 1));

            // 1. Khởi tạo AppState dùng chung
            let state = AppState {
                config: Arc::new(config.clone()),
                client: reqwest::Client::new(),
                jwt_decoding_key: Arc::new(decoding_key),
                signing_key: signing_key,
                rate_limiter,
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
