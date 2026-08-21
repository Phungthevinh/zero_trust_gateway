// =====================================================================
// Project: Zero-Trust API Gateway
// Author: Phung The Vinh (ptvstar2003@gmail.com)
// Copyright © 2026. All rights reserved.
// =====================================================================

use crate::proxy::AppState;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{Json, extract::State};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::IntervalStream;

#[allow(dead_code)]
#[derive(Default)]
pub struct GatewayMetrics {
    // Tổng số request được xử lý
    pub total_requests: AtomicUsize,
    // Số requests đang được xử lý ngay lúc này
    pub active_requests: AtomicUsize,
    //số request trả về lỗi 4xx, 5xx
    pub total_errors: AtomicUsize,
    // số lần sematic Cache trúng đích
    pub ai_cache_hits: AtomicUsize,
    // số lần sematic Cache trượt
    pub ai_cache_misses: AtomicUsize,
}

#[derive(serde::Serialize)]
pub struct MetricsSnapshot {
    pub total_requests: usize,
    pub active_requests: usize,
    pub total_errors: usize,
    pub ai_cache_hits: usize,
    pub ai_cache_misses: usize,
}

impl GatewayMetrics {
    pub fn get_snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            active_requests: self.active_requests.load(Ordering::Relaxed),
            total_errors: self.total_errors.load(Ordering::Relaxed),
            ai_cache_hits: self.ai_cache_hits.load(Ordering::Relaxed),
            ai_cache_misses: self.ai_cache_misses.load(Ordering::Relaxed),
        }
    }

    pub fn record_active_request(self: &Arc<Self>) -> ActiveRequestGuard {
        self.active_requests.fetch_add(1, Ordering::Relaxed);
        ActiveRequestGuard {
            metrics: Arc::clone(self),
        }
    }
}

pub async fn admin_metrics_handler(State(state): State<AppState>) -> Json<MetricsSnapshot> {
    Json(state.metrics.get_snapshot())
}

pub async fn admin_metrics_sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let interval = tokio::time::interval(Duration::from_secs(1));
    let stream = IntervalStream::new(interval).map(move |_| {
        let snapshot = state.metrics.get_snapshot();
        let json_str = serde_json::to_string(&snapshot).unwrap_or_default();
        Ok(Event::default().data(json_str))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

pub struct ActiveRequestGuard {
    metrics: Arc<GatewayMetrics>,
}

impl Drop for ActiveRequestGuard {
    fn drop(&mut self) {
        self.metrics.active_requests.fetch_sub(1, Ordering::Relaxed);
    }
}
