use crate::config::Config;

use axum::{body::Body, http::Request};
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::RwLock;

pub struct FastRejectFilter {
    ip_blacklist: Arc<RwLock<HashSet<String>>>,
    blocked_paths: Vec<String>,
    max_header_count: usize,
    max_uri_length: usize,
    max_body_size: usize,
}

impl FastRejectFilter {
    pub fn new(config: &Config) -> Self {
        Self {
            ip_blacklist: Arc::new(RwLock::new(HashSet::new())),
            blocked_paths: config.security.fast_reject.blocked_paths.clone(),
            max_header_count: config.security.fast_reject.max_header_count,
            max_uri_length: config.security.fast_reject.max_uri_length,
            max_body_size: config.security.fast_reject.max_body_size,
        }
    }

    pub fn check_request(&self, req: &Request<Body>) -> Result<(), String> {
        if let Some(forwarded_for) = req.headers().get("x-forwarded-for") {
            if let Ok(ip_str) = forwarded_for.to_str() {
                let client_ip = ip_str.split(',').next().unwrap_or("").trim();

                if self.ip_blacklist.read().unwrap().contains(client_ip) {
                    return Err("IP address is blacklisted".to_string());
                }
            }
        }
        Ok(())
    }
}
