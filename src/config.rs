// =====================================================================
// Project: Zero-Trust API Gateway
// Author: Phung The Vinh (ptvstar2003@gmail.com)
// Copyright © 2026. All rights reserved.
// =====================================================================

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;

//nơi chứa toàn bộ các khối cấu hình
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub security: SecurityConfig,
    pub ai_native: AiNativeConfig,
    pub routes: Vec<RouteConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub log_level: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DatabaseConfig {
    pub redis_url: String,
    pub connection_timeout: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SecurityConfig {
    pub jwt: JwtConfig,
    pub zero_trust: ZeroTrustConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JwtConfig {
    pub secret_key_path: String,
    pub issuer: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ZeroTrustConfig {
    pub private_key_path: String,
    pub signature_header: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AiNativeConfig {
    pub model_path: String,
    pub similarity_threshold: f32,
    pub cache_ttl: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RouteConfig {
    pub path: String,
    pub target: String,
    pub strip_prefix: bool,
    pub auth_required: bool,
    pub rate_limit: Option<RateLimitConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RateLimitConfig {
    pub max_requests: u64,
    pub per_seconds: u64,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let config: Config = serde_yaml::from_str(&contents)?;
        Ok(config)
    }
}
