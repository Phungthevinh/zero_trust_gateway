// =====================================================================
// Project: Zero-Trust API Gateway
// Author: Phung The Vinh (ptvstar2003@gmail.com)
// Copyright © 2026. All rights reserved.
// =====================================================================

use moka::future::Cache;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct BucketState {
    pub tokens: f64, // Số token hiện tại (dùng f64 để tính token lẻ theo thời gian)
    pub last_update: Instant, // Thời điểm cập nhật cuối cùng
}

pub struct RateLimiter {
    cache: Cache<String, Arc<Mutex<BucketState>>>,
    capacity: f64,
    refill_rate: f64, // Số token được thêm vào mỗi giây
}

impl RateLimiter {
    pub fn new(capacity: f64, refill_rate: f64, ttl_seconds: u64) -> Self {
        let cache = Cache::builder()
            .max_capacity(10_000)
            .time_to_live(Duration::from_secs(ttl_seconds))
            .time_to_idle(Duration::from_secs(10 * 60))
            .build();

        Self {
            cache,
            capacity,
            refill_rate,
        }
    }

    pub async fn check_request(&self, key: &str) -> bool {
        let now = Instant::now();
        // Sử dụng `get_with` để đảm bảo thao tác kiểm tra và khởi tạo là nguyên tử (Atomic)
        // Tránh tình trạng Race Condition (TOCTOU) khi nhiều request cùng gọi lúc cache rỗng.
        let state_arc = self
            .cache
            .get_with(key.to_string(), async {
                Arc::new(Mutex::new(BucketState {
                    tokens: self.capacity,
                    last_update: now,
                }))
            })
            .await;

        let mut state = state_arc.lock().unwrap();
        // tính toán thời gian đã trôi qua kể từ lần last_update
        let elapsed = now.duration_since(state.last_update).as_secs_f64();

        //Cộng thêm token dựa trên refill_rate * thời_gian_trôi_qua.
        state.tokens += self.refill_rate * elapsed;
        state.last_update = now;

        //Đảm bảo số token không vượt quá capacity.
        if state.tokens > self.capacity {
            state.tokens = self.capacity
        }

        //Nếu số token >= 1.0 -> Trừ đi 1, lưu lại cache, return true.
        if state.tokens >= 1.0 {
            state.tokens -= 1.0;
            state.last_update = now;
            true
        } else {
            false
        }
    }
}
