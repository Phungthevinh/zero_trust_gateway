# ⚡ Zero-Trust API Gateway & AI-Native Semantic Cache

<p align="center">
  <img src="https://img.shields.io/badge/Language-Rust-orange?style=for-the-badge&logo=rust" alt="Rust" />
  <img src="https://img.shields.io/badge/Framework-Axum%200.8-blue?style=for-the-badge&logo=cargo" alt="Axum" />
  <img src="https://img.shields.io/badge/Runtime-Tokio-darkblue?style=for-the-badge&logo=tokio" alt="Tokio" />
  <img src="https://img.shields.io/badge/License-MIT-green?style=for-the-badge" alt="License" />
  <img src="https://img.shields.io/badge/PRs-Welcome-brightgreen?style=for-the-badge" alt="PRs Welcome" />
</p>

---

## 🌟 Giới thiệu tổng quan (Overview)

**Zero-Trust API Gateway** là giải pháp cổng kiểm soát bảo mật thế hệ mới hiệu năng cao được viết hoàn toàn bằng **Rust**. Dự án được thiết kế đặc biệt cho mô hình Solo Developer (B2B / Open-Core) giúp tối ưu hóa hiệu năng, giảm thiểu lượng tiêu thụ tài nguyên RAM (chỉ từ 15MB so với 500MB+ của các giải pháp truyền thống như Kong) và tích hợp các tính năng đột phá liên quan đến tối ưu chi phí sử dụng AI (AI-Native).

Hệ thống hoạt động theo triết lý bảo mật tối tân: **"Never Trust, Always Verify"** (Không tin tưởng ai, luôn luôn xác thực).

---

## ✨ Tính năng nổi bật (Key Features)

* **🚀 Lõi hiệu năng vượt trội**: Xây dựng trên nền tảng `Axum 0.8` & `Tokio Async Engine` giúp xử lý hàng trăm ngàn requests/giây với độ trễ tối thiểu (sub-millisecond latency).
* **🔒 Bảo mật Zero-Trust chặt chẽ**:
  * Tích hợp bộ lọc Middleware xác thực JWT mạnh mẽ.
  * Tự động ký số nội bộ (Internal Signature) bằng mật mã bất đối xứng Ed25519 (`ring`) trước khi chuyển tiếp sang mạng nội bộ, ngăn chặn giả mạo nguồn gốc từ bên ngoài.
* **🛡️ Rate Limiting thông minh**: Hỗ trợ thuật toán *Token Bucket* / *Leaky Bucket* cực nhanh qua bộ nhớ đệm `moka` cục bộ, kết hợp đồng bộ hóa đa cụm (cluster) qua `redis`.
* **🧠 Trí tuệ nhân tạo (AI-Native Gateway)**:
  * Tích hợp proxy điều phối luồng truy cập LLM (OpenAI/Gemini).
  * Chạy mô hình Embedding ONNX trực tiếp (offline) tại Gateway bằng `tract-onnx` để phân tích ngữ nghĩa câu hỏi.
  * Bộ đệm ngữ nghĩa **Semantic Cache** giúp tái sử dụng câu trả lời tương tự từ bộ nhớ cục bộ, tiết kiệm tới **70% chi phí gọi API LLM**.

---

## 🗺️ Kiến trúc hệ thống (System Architecture)

Dưới đây là mô hình luồng đi của dữ liệu qua Gateway:

```mermaid
graph TD
    Client[Client / Public User] -->|1. Request HTTP| Gateway[Zero-Trust Gateway]
    
    subgraph Gateway [Zero-Trust API Gateway]
        Auth[JWT & Security Middleware] -->|2. Verify| RateLimit[Rate Limiter: Moka & Redis]
        RateLimit -->|3. Route Match| AICache{AI-Native Routing?}
        AICache -->|Yes| VectorCache[ONNX Embedding & Semantic Cache]
        AICache -->|No| RevProxy[Reverse Proxy Engine]
    end
    
    VectorCache -->|Cache Hit| Client
    VectorCache -->|Cache Miss| LLM[OpenAI / Gemini APIs]
    RevProxy -->|4. Add Ed25519 Signature| Upstream[Internal Upstream Services]
    
    style Gateway fill:#1f2335,stroke:#7aa2f7,stroke-width:2px,color:#fff
    style Upstream fill:#1a1b26,stroke:#41a2f6,stroke-width:1px,color:#fff
    style LLM fill:#1a1b26,stroke:#bb9af7,stroke-width:1px,color:#fff
```

---

## 🛠️ Công nghệ sử dụng (Tech Stack)

| Thành phần | Crate Khuyên Dùng | Mô tả kỹ thuật |
| :--- | :--- | :--- |
| **Async Engine** | `tokio 1.52` | Xử lý đa luồng non-blocking cực mạnh |
| **Routing & Server** | `axum 0.8` | Định tuyến API tốc độ cao, type-safe |
| **Security** | `jsonwebtoken 10`, `ring 0.17` | Mã hóa JWT, ký số Ed25519 bảo vệ microservices |
| **Caching/Session** | `moka 0.12`, `redis 1.2` | Cache cục bộ in-memory và phân tán qua Redis |
| **AI Processing** | `tract-onnx 0.21`, `reqwest` | Chạy ONNX embedding và proxy LLM traffic |

---

## 🚀 Hướng dẫn khởi chạy nhanh (Quick Start)

### 1. Yêu cầu hệ thống (Prerequisites)
* Đã cài đặt **Rust** (bản ổn định mới nhất, tối thiểu `v1.75`).
* (Tùy chọn) Máy chủ **Redis** nếu chạy đa cụm.

### 2. Cài đặt (Installation)
Tải mã nguồn về máy cục bộ:
```bash
git clone https://github.com/Phungthevinh/zero_trust_gateway.git
cd zero_trust_gateway/zero_trust_gateway
```

### 3. Cấu hình (Configuration)
Tạo file cấu hình `config.yaml` tại thư mục gốc của dự án:
```yaml
server:
  host: "0.0.0.0"
  port: 8080

routes:
  - path: "/api/v1/users"
    target: "http://localhost:8081"
  - path: "/api/v1/orders"
    target: "http://localhost:8082"
```

### 4. Biên dịch và chạy (Run)
Khởi chạy Gateway ở chế độ phát triển (Development):
```bash
cargo run
```

Để biên dịch bản Release tối ưu hóa hiệu năng cao nhất:
```bash
cargo build --release
./target/release/zero_trust_gateway
```

### 5. Thử nghiệm hiệu năng & Kiểm tra độ trễ (Latency & Benchmark Testing)
Hệ thống tích hợp sẵn kịch bản kiểm thử đo đạc hiệu năng chuyển tiếp gói tin (Reverse Proxy latency) thông qua mock upstream server chạy ngầm. Để chạy thử nghiệm hiệu năng và xem báo cáo độ trễ chi tiết, bạn hãy chạy lệnh sau:
```bash
cargo test -- --nocapture
```

**Báo cáo kiểm thử thực tế trên localhost (Chế độ Debug):**
* **Số lượng request**: 100 requests liên tục.
* **Độ trễ trung bình**: **~251.7 μs** (tương đương **~0.25 ms**).
* **Tỉ lệ thành công**: 100% (HTTP 200 OK).

---


## 📅 Tiến độ dự án (Roadmap)
Xem chi tiết trạng thái triển khai tại [PROGRESS.md](PROGRESS.md).

## 🤝 Đóng góp ý kiến (Contributing)
Mọi đóng góp, báo lỗi (issues) và yêu cầu tính năng (PRs) đều được chào đón! Hãy mở một Pull Request hoặc Issue để chúng ta cùng thảo luận.

## 📄 Giấy phép (License)
Dự án được phân phối dưới giấy phép **MIT**. Xem tệp [LICENSE](LICENSE) để biết thêm chi tiết.

---

## 👤 Tác giả (Author)
* **Phùng Thế Vinh** - *Chủ dự án & Nhà phát triển chính* - [@Phungthevinh](https://github.com/Phungthevinh)
* Email liên hệ: ptvstar2003@gmail.com

