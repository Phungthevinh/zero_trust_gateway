# Tiến độ Dự án: Zero-Trust API Gateway

Tệp này được sử dụng để theo dõi tiến độ triển khai thực tế của dự án qua các giai đoạn khác nhau.

---

## 📊 Trạng thái Hiện tại
* **Giai đoạn Hiện tại**: Giai đoạn 1: Lõi hiệu năng (Core Engine) & Reverse Proxy
* **Trạng thái**: 🟡 Đang lập trình bộ đọc cấu hình (config parser) & Web Server
* **Cập nhật gần nhất**: 11/06/2026

---

## 🗺️ Lộ trình Phát triển (Development Roadmap)

### 🟡 Giai đoạn 1: Xây dựng Lõi hiệu năng (Core Engine) & Reverse Proxy (Tháng 1-2)
- [x] Thiết lập cấu hình ban đầu và cài đặt thư viện cần thiết (`Cargo.toml`)
- [x] Thiết kế và lập cấu trúc file cấu hình `config.yaml` cho Gateway
- [/] Xây dựng Web Server cơ bản sử dụng `axum` & `tokio` (và bộ đọc cấu hình)
- [ ] Triển khai Reverse Proxy Middleware chuyển tiếp request sang cổng dịch vụ Upstream
- [ ] Kiểm tra tối ưu hóa rò rỉ bộ nhớ (memory leaks) và đo đạc hiệu năng cơ bản

### ⚪ Giai đoạn 2: Tích hợp Lưới bảo mật Zero-Trust & Rate Limiting (Tháng 3-4)
- [ ] Viết Middleware xác thực Token JWT (`jsonwebtoken`)
- [ ] Thiết lập cơ chế chữ ký nội bộ (Internal Signature) bằng mật mã Ed25519 (`ring`)
- [ ] Xây dựng bộ lọc Rate Limiting cục bộ với thuật toán Token/Leaky Bucket sử dụng `moka`
- [ ] Tích hợp `redis` để đồng bộ hóa Rate Limiting giữa các cụm Gateway
- [ ] Thiết lập cơ chế tự động từ chối request xấu siêu tốc dưới 1.2ms

### ⚪ Giai đoạn 3: Tính năng Đột phá - AI-Native Gateway (Tháng 5)
- [ ] Tạo Proxy phân phối và điều phối lưu lượng truy cập AI
- [ ] Tích hợp mô hình AI ONNX nhúng cục bộ thông qua `tract-onnx`
- [ ] Xây dựng Vector Cache trong bộ nhớ để triển khai cơ chế Semantic Cache (tiết kiệm chi phí gọi LLM)
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
