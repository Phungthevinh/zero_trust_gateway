// =====================================================================
// Project: Zero-Trust API Gateway
// Author: Phung The Vinh (ptvstar2003@gmail.com)
// Copyright © 2026. All rights reserved.
// =====================================================================

use chrono::Utc;
use redis::Script;

pub struct RedisRateLimiter {
    client: redis::Client,
}

const RATE_LIMIT_SCRIPT: &str = r#"
            local key = KEYS[1]
            local limit = tonumber(ARGV[1])
            local window = tonumber(ARGV[2])
            local now = tonumber(ARGV[3])

            redis.call('ZREMRANGEBYSCORE', key, 0, now - window)
            local count = redis.call('ZCARD', key)

            if count < limit then
                redis.call('ZADD', key, now, now .. '-' .. math.random())
                redis.call('EXPIRE', key, window)
                return 1  -- Cho phép
            else
                return 0  -- Từ chối
            end
            "#;
impl RedisRateLimiter {
    pub fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        Ok(Self { client })
    }

    pub async fn check_request(
        &self,
        key: &str,
        max_requests: u64,
        window_seconds: u64,
    ) -> Result<bool, redis::RedisError> {
        let now = Utc::now().timestamp();
        //lấy connection từ client
        let mut con = self.client.get_multiplexed_async_connection().await?;
        let script = Script::new(RATE_LIMIT_SCRIPT);

        let result: i32 = script
            .key(key)
            .arg(max_requests)
            .arg(window_seconds)
            .arg(now)
            .invoke_async(&mut con)
            .await?;

        Ok(result == 1)
    }
}
