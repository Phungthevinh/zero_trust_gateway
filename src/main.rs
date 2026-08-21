// =====================================================================
// Project: Zero-Trust API Gateway
// Author: Phung The Vinh (ptvstar2003@gmail.com)
// Copyright © 2026. All rights reserved.
// =====================================================================

// === Core Framework ===
use axum::{Router, routing::get};
use std::sync::Arc;

// === Configuration ===
mod config;
use config::Config;

// === Security Modules ===
mod auth;
mod fast_reject;
mod signature;
use fast_reject::FastRejectFilter;
use jsonwebtoken::DecodingKey;

// === Rate Limiting ===
mod rate_limit;
mod redis_rate_limit;

// === AI-Native Engine ===
mod ai_engine;
mod semantic_cache;
use ai_engine::AiEngine;
use semantic_cache::SemanticCache;

// === Monitoring & Dashboard ===
mod metrics;
mod dashboard;
use metrics::{admin_metrics_handler, admin_metrics_sse_handler};

// === Reverse Proxy ===
mod proxy;
use proxy::{AppState, proxy_handler};

// =====================================================================
// Helper: In thông tin cấu hình khi khởi động Gateway
// =====================================================================
fn print_startup_banner(config: &Config) {
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
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    match Config::load("config.yaml") {
        Ok(config) => {
            print_startup_banner(&config);

            // Khởi tạo Fast Reject Filter
            let fast_reject = Arc::new(FastRejectFilter::new(&config));

            // Khởi tạo JWT Decoding Key (RSA Public Key)
            let public_key_pem = match std::fs::read(&config.security.jwt.secret_key_path) {
                Ok(bytes) => bytes,
                Err(e) => {
                    tracing::error!("Lỗi khi đọc public key: {}", e);
                    return;
                }
            };
            let decoding_key = DecodingKey::from_rsa_pem(&public_key_pem).unwrap();

            // Khởi tạo Ed25519 Signing Key cho Zero-Trust
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

            // Khởi tạo Rate Limiter cục bộ (Token Bucket: 100 req/s, refill 10 req/s, TTL 1s)
            let rate_limiter = Arc::new(rate_limit::RateLimiter::new(100.0, 10.0, 1));

            // Khởi tạo Semantic Cache (Graceful Degradation nếu model không nạp được)
            let semantic_cache = match AiEngine::new(&config.ai_native.model_path) {
                Ok(ai_engine) => {
                    let cache = SemanticCache::new(
                        Arc::new(ai_engine),
                        config.ai_native.similarity_threshold,
                        config.ai_native.cache_ttl,
                    );
                    Some(Arc::new(cache))
                }
                Err(e) => {
                    tracing::warn!("Không thể khởi tạo Semantic Cache: {:?}", e);
                    None
                }
            };

            // 1. Khởi tạo AppState chia sẻ giữa các handler
            let state = AppState {
                config: Arc::new(config.clone()),
                client: reqwest::Client::new(),
                jwt_decoding_key: Arc::new(decoding_key),
                signing_key,
                rate_limiter,
                fast_reject,
                semantic_cache,
                metrics: Arc::new(metrics::GatewayMetrics::default()),
            };

            // 2. Dựng Router và gắn State
            let app = Router::new()
                .route("/health", get(|| async { "OK" }))
                .route("/admin/metrics", get(admin_metrics_handler))
                .route("/admin/events", get(admin_metrics_sse_handler))
                .route("/dashboard", get(dashboard::dashboard_handler))
                .route("/dashboard/{*path}", get(dashboard::dashboard_handler))
                .fallback(proxy_handler)
                .with_state(state);

            // 3. Lắng nghe và khởi chạy Server
            let addr = format!("{}:{}", config.server.host, config.server.port);
            let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

            tracing::info!("API Gateway đang chạy trên http://{}", addr);
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await
            .unwrap();
        }
        Err(e) => {
            eprintln!("Lỗi khi tải cấu hình: {}", e);
        }
    }
}
