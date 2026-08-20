# Tiến độ Dự án: Zero-Trust API Gateway

Tệp này được sử dụng để theo dõi tiến độ triển khai thực tế của dự án qua các giai đoạn khác nhau.

---

## 📊 Trạng thái Hiện tại
* **Giai đoạn Hiện tại**: Giai đoạn 4: Hệ thống Giám sát, Giao diện Quản trị (UI) & Đóng gói (Tháng 6)
* **Trạng thái**: 🟡 Đang thực hiện Giai đoạn 4 — Hoàn thành Metrics API, SSE Stream & Web Dashboard
* **Cập nhật gần nhất**: 20/08/2026

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
- [x] Triển khai cơ chế xác định Client IP an toàn với `Trusted Proxies` chống IP Spoofing cho Rate Limiter

### 🟢 Giai đoạn 3: Tính năng Đột phá - AI-Native Gateway (Tháng 5)
- [x] Xây dựng AI Embedding Engine (`src/ai_engine.rs`) sử dụng `tract-onnx` để chạy mô hình `all-MiniLM-L6-v2` nhúng cục bộ, chuyển đổi text thành vector 384 chiều với Mean Pooling & L2 Normalization (Cosine Similarity A vs B đạt 0.99)
- [x] Xây dựng Vector Cache trong bộ nhớ để triển khai cơ chế Semantic Cache (`src/semantic_cache.rs`) — Bao gồm: `CacheEntry` struct, `SemanticCache` struct với `RwLock<Vec>`, `new()`, `cosine_similarity()` (dot product cho L2-normalized vectors), `lookup()` (TTL check + best-match), `insert()` (embed + retain cleanup + push), `cleanup_expired()`. Unit test PASS: Cache HIT (câu tương đồng) + Cache MISS (TTL expired)
- [x] Tích hợp `AiEngine` + `SemanticCache` vào `AppState` (Dependency Injection) trong `main.rs` — Khởi tạo Graceful Degradation: nếu model ONNX không nạp được thì Gateway vẫn hoạt động bình thường với `semantic_cache = None`. Cập nhật toàn bộ Unit Test trong `proxy.rs` (thêm `semantic_cache: None`, sửa `Host` header cho FastReject, nâng capacity RateLimiter cho latency test). Tất cả 4/4 test PASS ✅
- [x] Tạo Proxy phân phối và điều phối lưu lượng truy cập AI (AI Traffic Branching trong `proxy_handler`)
- [x] Tối ưu hóa bộ nhớ đệm và parse giao thức MCP (Model Context Protocol)

### 🟡 Giai đoạn 4: Hệ thống Giám sát, Giao diện Quản trị (UI) & Đóng gói (Tháng 6)
- [x] Xây dựng Core Metrics Collector (`src/metrics.rs`) với `GatewayMetrics` struct sử dụng `AtomicUsize` theo dõi: `total_requests`, `active_requests`, `total_errors`, `ai_cache_hits`, `ai_cache_misses`. Triển khai RAII Guard (`ActiveRequestGuard` + trait `Drop`) đảm bảo `active_requests` luôn chính xác 100% kể cả khi request panic hoặc return sớm. Tích hợp vào `proxy_handler` và `AppState`.
- [x] Xây dựng API REST (`GET /admin/metrics` → JSON snapshot) và Real-time SSE Stream (`GET /admin/events` → Server-Sent Events mỗi 1 giây) sử dụng `IntervalStream` + `KeepAlive` của Axum. Thêm dependencies: `tokio-stream`, `futures-util`.
- [x] Thiết kế và xây dựng Web Dashboard quản trị (`static/index.html`, `style.css`, `app.js`) — Giao diện Dark Glassmorphism hiện đại với: 4 thẻ chỉ số thời gian thực (Total Requests, Active In-Flight, Errors, AI Cache Hit Ratio %), biểu đồ đường Chart.js cập nhật 1s/lần, bảng chi tiết AI Semantic Cache Hit/Miss, thanh tiến trình Cache Efficiency, hiển thị trạng thái kết nối SSE (Live/Reconnecting). Phục vụ tĩnh qua `tower-http::ServeDir` tại route `/dashboard`.
- [ ] Đóng gói dự án thành Single Binary duy nhất bằng `rust-embed` (nhúng static assets vào binary) và tối ưu hóa file thực thi
- [ ] Triển khai đo tải (Benchmarking) so sánh với Nginx và Kong

---
*Ghi chú ký hiệu trạng thái:*
* 🟢 **Hoàn thành (Done)**
* 🟡 **Đang thực hiện (In-progress)**
* ⚪ **Chưa bắt đầu (Pending)**
