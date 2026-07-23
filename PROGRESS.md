# Tiến độ Dự án: Zero-Trust API Gateway

Tệp này được sử dụng để theo dõi tiến độ triển khai thực tế của dự án qua các giai đoạn khác nhau.

---

## 📊 Trạng thái Hiện tại
* **Giai đoạn Hiện tại**: Giai đoạn 3: Tính năng Đột phá - AI-Native Gateway (Tháng 5)
* **Trạng thái**: 🟡 Đang thực hiện Giai đoạn 3 — Semantic Cache đã tích hợp vào AppState ✅, tiếp theo: AI Proxy Dispatcher Logic trong proxy_handler
* **Cập nhật gần nhất**: 23/07/2026

---

## 🗺️ Lộ trình Phát triển (Development Roadmap)

### 🟢 Giai đoạn 1: Xây dựng Lõi hiệu năng (Core Engine) & Reverse Proxy (Tháng 1-2)
- [x] Thiết lập cấu hình ban đầu và cài đặt thư viện cần thiết (`Cargo.toml`)
- [x] Thiết kế và lập cấu trúc file cấu hình `config.yaml` cho Gateway
- [x] Viết struct và bộ đọc cấu hình trong `src/config.rs`
- [x] Xây dựng Web Server cơ bản sử dụng `axum` & `tokio`
- [x] Triển khai Reverse Proxy Middleware chuyển tiếp request sang cổng dịch vụ Upstream
- [x] Kiểm tra tối ưu hóa rò rỉ bộ nhớ (memory leaks) và đo đạc hiệu năng cơ bản

### 🟢 Giai đoạn 2: Tích hợp Lưới bảo mật Zero-Trust & Rate Limiting (Tháng 3-4)
- [x] Viết Middleware xác thực Token JWT (`jsonwebtoken`)
- [x] Thiết lập cơ chế chữ ký nội bộ (Internal Signature) bằng mật mã Ed25519 (`ring`)
- [x] Xây dựng bộ lọc Rate Limiting cục bộ với thuật toán Token/Leaky Bucket sử dụng `moka`
- [x] Tích hợp `redis` để đồng bộ hóa Rate Limiting giữa các cụm Gateway
- [x] Thiết lập cơ chế tự động từ chối request xấu siêu tốc dưới 1.2ms (`FastRejectFilter`)

### 🟡 Giai đoạn 3: Tính năng Đột phá - AI-Native Gateway (Tháng 5)
- [x] Xây dựng AI Embedding Engine (`src/ai_engine.rs`) sử dụng `tract-onnx` để chạy mô hình `all-MiniLM-L6-v2` nhúng cục bộ, chuyển đổi text thành vector 384 chiều với Mean Pooling & L2 Normalization (Cosine Similarity A vs B đạt 0.99)
- [x] Xây dựng Vector Cache trong bộ nhớ để triển khai cơ chế Semantic Cache (`src/semantic_cache.rs`) — Bao gồm: `CacheEntry` struct, `SemanticCache` struct với `RwLock<Vec>`, `new()`, `cosine_similarity()` (dot product cho L2-normalized vectors), `lookup()` (TTL check + best-match), `insert()` (embed + retain cleanup + push), `cleanup_expired()`. Unit test PASS: Cache HIT (câu tương đồng) + Cache MISS (TTL expired)
- [x] Tích hợp `AiEngine` + `SemanticCache` vào `AppState` (Dependency Injection) trong `main.rs` — Khởi tạo Graceful Degradation: nếu model ONNX không nạp được thì Gateway vẫn hoạt động bình thường với `semantic_cache = None`. Cập nhật toàn bộ Unit Test trong `proxy.rs` (thêm `semantic_cache: None`, sửa `Host` header cho FastReject, nâng capacity RateLimiter cho latency test). Tất cả 4/4 test PASS ✅
- [ ] Tạo Proxy phân phối và điều phối lưu lượng truy cập AI (AI Traffic Branching trong `proxy_handler`)
- [ ] Tối ưu hóa bộ nhớ đệm và parse giao thức MCP (Model Context Protocol)

### ⚪ Giai đoạn 4: Thiết kế Giao diện quản trị (UI) & Đóng gói (Tháng 6)
- [ ] Phát triển API quản trị nội bộ để xuất các luồng dữ liệu traffic (REST/SSE/WebSockets)
- [ ] Thiết lập giao diện Web Dashboard quản trị (React/Vue template) hiển thị biểu đồ thời gian thực
- [ ] Đóng gói dự án thành Single Binary duy nhất và tối ưu hóa file thực thi
- [ ] Triển khai đo tải (Benchmarking) so sánh với Nginx và Kong

---
*Ghi chú ký hiệu trạng thái:*
* 🟢 **Hoàn thành (Done)**
* 🟡 **Đang thực hiện (In-progress)**
* ⚪ **Chưa bắt đầu (Pending)**
