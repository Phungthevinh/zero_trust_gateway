// =====================================================================
// Project: Zero-Trust API Gateway
// Author: Phung The Vinh (ptvstar2003@gmail.com)
// Copyright © 2026. All rights reserved.
// =====================================================================

mod config;
use config::Config;

fn main() {
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
        }
        Err(e) => {
            eprintln!("Lỗi khi tải cấu hình: {}", e);
        }
    }
}
