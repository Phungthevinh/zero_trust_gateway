// =====================================================================
// Project: Zero-Trust API Gateway
// Author: Phung The Vinh (ptvstar2003@gmail.com)
// Copyright © 2026. All rights reserved.
// =====================================================================

use crate::config::Config;

use axum::http::{Method, header, request::Parts};
use std::collections::HashSet;
use std::fmt::Display;
use std::sync::Arc;
use std::sync::RwLock;

#[derive(Debug)]
pub enum RejectReason {
    BlacklistedIp(String),  // kèm IP bị chặn
    SuspiciousPath(String), // kèm path bị chặn
    TooManyHeaders(usize),  // kèm số headers
    UriTooLong(usize),      // kèm độ dài URI
    MissingHostHeader,
    InvalidMethod(String), // kèm method không hợp lệ
    BodyTooLarge(usize),
}

impl Display for RejectReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlacklistedIp(ip) => write!(f, "Blacklisted IP: {}", ip),
            Self::SuspiciousPath(path) => write!(f, "Suspicious Path: {}", path),
            Self::TooManyHeaders(count) => write!(f, "Too Many Headers: {}", count),
            Self::UriTooLong(length) => write!(f, "Uri Too Long: {}", length),
            Self::MissingHostHeader => write!(f, "Missing Host Header"),
            Self::InvalidMethod(method) => write!(f, "Invalid Method: {}", method),
            Self::BodyTooLarge(size) => write!(f, "Body Too Large: {}", size),
        }
    }
}

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
            ip_blacklist: Arc::new(RwLock::new(
                config
                    .security
                    .fast_reject
                    .ip_blacklist
                    .iter()
                    .cloned()
                    .collect(),
            )),
            blocked_paths: config.security.fast_reject.blocked_paths.clone(),
            max_header_count: config.security.fast_reject.max_header_count,
            max_uri_length: config.security.fast_reject.max_uri_length,
            max_body_size: config.security.fast_reject.max_body_size,
        }
    }

    pub fn check_request(&self, req: &Parts) -> Result<(), RejectReason> {
        if let Some(forwarded_for) = req.headers.get("x-forwarded-for") {
            if let Ok(ip_str) = forwarded_for.to_str() {
                let client_ip = ip_str.split(',').next().unwrap_or("").trim();

                if self.ip_blacklist.read().unwrap().contains(client_ip) {
                    return Err(RejectReason::BlacklistedIp(client_ip.to_string()));
                }
            }
        }

        if req.uri.path().len() > self.max_uri_length {
            return Err(RejectReason::UriTooLong(req.uri.path().len()));
        }

        let current_method = &req.method;
        match *current_method {
            Method::GET => {}
            Method::PATCH => {}
            Method::POST => {}
            Method::PUT => {}
            Method::DELETE => {}
            Method::HEAD => {}
            Method::OPTIONS => {}
            _ => {
                return Err(RejectReason::InvalidMethod(req.method.to_string()));
            }
        }

        if req.headers.get(header::HOST).is_none() {
            return Err(RejectReason::MissingHostHeader);
        }

        let header_count = req.headers.len() as usize;
        if header_count > self.max_header_count {
            return Err(RejectReason::TooManyHeaders(header_count));
        }

        // Thêm vào check_request, SAU check header count, TRƯỚC check path pattern:
        if let Some(content_length) = req.headers.get(header::CONTENT_LENGTH) {
            if let Ok(len_str) = content_length.to_str() {
                if let Ok(len) = len_str.parse::<usize>() {
                    if len > self.max_body_size {
                        return Err(RejectReason::BodyTooLarge(len));
                    }
                }
            }
        }

        let pattern = req.uri.path();
        for blocked in &self.blocked_paths {
            if pattern.starts_with(blocked) {
                return Err(RejectReason::SuspiciousPath(blocked.clone()));
            }
        }
        Ok(())
    }
}
