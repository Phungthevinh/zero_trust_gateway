use std::time::Instant;
use std::sync::RwLock;
use std::sync::Arc;
use std::time::Duration;
use crate::ai_engine::AiEngine;

// 1. CacheEntry - Đại diện cho 1 bản ghi trong cache
pub struct CacheEntry {
    pub query: String,           // Câu hỏi gốc
    pub embedding: Vec<f32>,     // Vector 384 chiều
    pub response: String,        // Response từ LLM đã cache
    pub created_at: Instant,     // Thời điểm tạo
}

// 2. SemanticCache - Bộ nhớ cache chính
pub struct SemanticCache {
    entries: RwLock<Vec<CacheEntry>>,   // Danh sách các entry (dùng RwLock cho concurrent read)
    ai_engine: Arc<AiEngine>,           // Engine để embed câu hỏi mới
    similarity_threshold: f32,          // Ngưỡng cosine similarity (từ config, mặc định 0.85)
    ttl: Duration,                      // Thời gian sống của cache entry
}

impl SemanticCache {
    // 2.1. new() - Khởi tạo
    pub fn new(ai_engine: Arc<AiEngine>, similarity_threshold: f32, ttl_seconds: u64) -> Self{
        Self {
            entries: RwLock::new(vec![]),
            ai_engine,
            similarity_threshold,
            ttl: Duration::from_secs(ttl_seconds),
        }
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32{
        let dot_product :f32 = a.iter().zip(b.iter()).map(|(x,y)| x * y).sum();
        dot_product
    }

    // 2.2. lookup() - Tìm kiếm trong cache
    //   - Embed câu hỏi mới thành vector
    //   - Duyệt tất cả entries, tính cosine similarity
    //   - Nếu có entry nào sim >= threshold VÀ chưa hết TTL → trả về response (Cache HIT)
    //   - Nếu không → trả về None (Cache MISS)
    pub fn lookup(&self, query: &str) -> Option<String>{
        let query_embedding = match self.ai_engine.embed(query) {
            Ok(vec) => vec,
            Err(e) => {
                eprintln!("SemanticCache: Lỗi khi embed query: {}", e);
                return None;
            },
        };

        let entries = self.entries.read().unwrap();
        let now = Instant::now();
        let mut best_response: Option<String> = None;
        let mut best_score = 0.0;

        for entry in entries.iter(){
            if now.duration_since(entry.created_at) > self.ttl{
                continue;
            }
            let sim = Self::cosine_similarity(&query_embedding, &entry.embedding);
            if sim >= self.similarity_threshold && sim > best_score{
                best_score = sim;
                best_response = Some(entry.response.clone());
            }
        }

        best_response
    }

    // 2.3. insert() - Thêm entry mới vào cache SAU khi gọi LLM thành công
    //   - Embed câu hỏi, lưu cả vector + response
    pub fn insert(&self, query: &str, response: String) {
        todo!("Vinh sẽ tự code hàm này")
    }

    // 2.5. cleanup_expired() - Xóa các entry đã hết TTL (gọi định kỳ hoặc khi insert)
    fn cleanup_expired(&self) {
        todo!("Vinh sẽ tự code hàm này")
    }
}


#[cfg(test)]
mod tests {
    // Test 1: Insert rồi lookup đúng câu → phải HIT
    // Test 2: Insert "What is weather?" rồi lookup "How is the weather?" → phải HIT (semantic match)
    // Test 3: Insert "What is weather?" rồi lookup "I love Rust" → phải MISS
    // Test 4: Insert rồi chờ hết TTL → phải MISS (test với TTL ngắn, ví dụ 1 giây)
}
