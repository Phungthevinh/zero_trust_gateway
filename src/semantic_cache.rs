// =====================================================================
// Project: Zero-Trust API Gateway
// Author: Phung The Vinh (ptvstar2003@gmail.com)
// Copyright © 2026. All rights reserved.
// =====================================================================

use crate::ai_engine::AiEngine;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;
use std::time::Instant;

// 1. CacheEntry - Đại diện cho 1 bản ghi trong cache
pub struct CacheEntry {
    pub query: String,       // Câu hỏi gốc
    pub embedding: Vec<f32>, // Vector 384 chiều
    pub response: String,    // Response từ LLM đã cache
    pub created_at: Instant, // Thời điểm tạo
}

// 2. SemanticCache - Bộ nhớ cache chính
pub struct SemanticCache {
    entries: RwLock<Vec<CacheEntry>>, // Danh sách các entry (dùng RwLock cho concurrent read)
    ai_engine: Arc<AiEngine>,         // Engine để embed câu hỏi mới
    similarity_threshold: f32,        // Ngưỡng cosine similarity (từ config, mặc định 0.85)
    ttl: Duration,                    // Thời gian sống của cache entry
}

impl SemanticCache {
    // 2.1. new() - Khởi tạo
    pub fn new(ai_engine: Arc<AiEngine>, similarity_threshold: f32, ttl_seconds: u64) -> Self {
        Self {
            entries: RwLock::new(vec![]),
            ai_engine,
            similarity_threshold,
            ttl: Duration::from_secs(ttl_seconds),
        }
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        dot_product
    }

    // 2.2. lookup() - Tìm kiếm trong cache
    //   - Embed câu hỏi mới thành vector
    //   - Duyệt tất cả entries, tính cosine similarity
    //   - Nếu có entry nào sim >= threshold VÀ chưa hết TTL → trả về response (Cache HIT)
    //   - Nếu không → trả về None (Cache MISS)
    pub fn lookup(&self, query: &str) -> Option<String> {
        let query_embedding = match self.ai_engine.embed(query) {
            Ok(vec) => vec,
            Err(e) => {
                eprintln!("SemanticCache: Lỗi khi embed query: {}", e);
                return None;
            }
        };

        let entries = self.entries.read().unwrap();
        let now = Instant::now();
        let mut best_response: Option<String> = None;
        let mut best_score = 0.0;

        for entry in entries.iter() {
            if now.duration_since(entry.created_at) > self.ttl {
                continue;
            }
            let sim = Self::cosine_similarity(&query_embedding, &entry.embedding);
            if sim >= self.similarity_threshold && sim > best_score {
                best_score = sim;
                best_response = Some(entry.response.clone());
            }
        }

        best_response
    }

    // 2.3. insert() - Thêm entry mới vào cache SAU khi gọi LLM thành công
    //   - Embed câu hỏi, lưu cả vector + response
    pub fn insert(&self, query: &str, response: String) {
        let query_embedding = match self.ai_engine.embed(query) {
            Ok(vec) => vec,
            Err(e) => {
                eprintln!("SemanticCache: Lỗi khi embed query: {}", e);
                return;
            }
        };
        let new_entry = CacheEntry {
            query: query.to_string(),
            embedding: query_embedding,
            response: response,
            created_at: Instant::now(),
        };

        let mut entries = self.entries.write().unwrap();
        let now = Instant::now();
        entries.retain(|e| now.duration_since(e.created_at) <= self.ttl);
        entries.push(new_entry);
    }

    // 2.5. cleanup_expired() - Xóa các entry đã hết TTL (gọi định kỳ hoặc khi insert)
    fn cleanup_expired(&self) {
        let mut entries = self.entries.write().unwrap();
        let now = Instant::now();
        entries.retain(|e| now.duration_since(e.created_at) <= self.ttl);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_semantic_cache_flow() {
        // 1. Khởi tạo AiEngine (đường dẫn tới thư mục models từ thư mục src)
        let engine = AiEngine::new("../models/all-MiniLM-L6-v2.onnx")
            .expect("Không thể nạp model ONNX.");
        let ai_engine = Arc::new(engine);

        // 2. Khởi tạo SemanticCache với threshold 0.98 và TTL 2 giây
        // Do thuật toán băm byte giả lập nên các câu luôn có độ tương đồng > 0.89.
        // Ta set 0.98 để test tính năng cache HIT.
        let cache = SemanticCache::new(ai_engine, 0.98, 2);

        // 3. Thêm dữ liệu vào
        cache.insert(
            "Thời tiết hôm nay ra sao?",
            "Trời nắng đẹp".to_string(),
        );

        // 4. Test HIT: Câu tương tự
        let hit_result = cache.lookup("Hôm nay thời tiết thế nào?");
        assert_eq!(hit_result, Some("Trời nắng đẹp".to_string()), "Kỳ vọng HIT vì câu tương đồng.");

        // 5. Test TTL: Ngủ 3 giây để quá thời hạn (2 giây)
        println!("Đang ngủ 3 giây để test TTL...");
        sleep(Duration::from_secs(3));

        // 6. Test MISS (TTL Expired)
        let expired_result = cache.lookup("Thời tiết hôm nay ra sao?");
        assert_eq!(expired_result, None, "Kỳ vọng MISS vì cache đã hết hạn.");
    }
}
