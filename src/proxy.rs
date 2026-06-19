use crate::auth;
use crate::config::Config;
use crate::signature;
use axum::{
    body::Body,
    extract::State,
    http::{Request, Response, StatusCode},
    response::IntoResponse,
};
use jsonwebtoken::DecodingKey;
use ring::signature::Ed25519KeyPair;
use std::sync::Arc;

// Định nghĩa AppState chia sẻ dữ liệu giữa các luồng
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub client: reqwest::Client,
    pub jwt_decoding_key: Arc<DecodingKey>,
    pub signing_key: Arc<Ed25519KeyPair>,
}

// Handler nhận request và khớp route
pub async fn proxy_handler(State(state): State<AppState>, req: Request<Body>) -> impl IntoResponse {
    let (parts, body) = req.into_parts();

    let path = parts.uri.path();

    // Duyệt qua danh sách routes để tìm route khớp
    let matched_route = state
        .config
        .routes
        .iter()
        .find(|r| path.starts_with(&r.path));

    match matched_route {
        Some(route) => {
            tracing::info!(
                "Khớp route cấu hình: {} -> Target: {}",
                route.path,
                route.target
            );

            if route.auth_required {
                let token = auth::extract_token_from_header(&parts.headers);
                match token {
                    None => return StatusCode::UNAUTHORIZED.into_response(),
                    Some(token_str) => {
                        match auth::verify_token(
                            &token_str,
                            &state.jwt_decoding_key,
                            &state.config.security.jwt.issuer,
                        ) {
                            Ok(token_data) => {
                                tracing::debug!(
                                    "Token JWT hợp lệ cho user: {}",
                                    token_data.claims.sub
                                );
                            }
                            Err(e) => {
                                tracing::warn!("Token JWT không hợp lệ: {:?}", e);
                                return StatusCode::UNAUTHORIZED.into_response();
                            }
                        }
                    }
                }
            }

            let mut target_path = path.to_string();
            if route.strip_prefix {
                if let Some(stripped) = path.strip_prefix(&route.path) {
                    target_path = stripped.to_string();
                }
            }

            // Đảm bảo target_path bắt đầu bằng dấu '/' để URL hợp lệ
            if !target_path.starts_with('/') {
                target_path = format!("/{}", target_path);
            }

            // Ghép host upstream và path (loại bỏ dấu '/' thừa ở cuối host nếu có)
            let mut target_url = format!("{}{}", route.target.trim_end_matches('/'), target_path);

            // Đính kèm Query String (ví dụ: ?search=rust) nếu client có gửi lên
            if let Some(query) = parts.uri.query() {
                target_url = format!("{}?{}", target_url, query);
            }

            // chuyển đổi Axum Body sang reqwest Body
            // Đọc body từ Axum thành Bytes (giới hạn tối đa 10MB để tránh tấn công cạn kiệt bộ nhớ)
            let body_bytes = match axum::body::to_bytes(body, 10 * 1024 * 1024).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    tracing::error!("Lỗi khi đọc request body: {:?}", e);
                    return (StatusCode::BAD_REQUEST, "Không thể đọc request body").into_response();
                }
            };

            let (signature_b64, timestamp) = signature::sign_request(
                &state.signing_key,
                parts.method.as_str(),
                &target_path,
                &body_bytes,
            )
            .await;

            // Khởi tạo request builder với Method và URL mới
            let mut upstream_req = state.client.request(parts.method, &target_url);

            // Sao chép toàn bộ headers sang request mới, TRỪ header HOST
            // Tạm thời trả về text để kiểm tra định tuyến

            for (header_name, header_value) in parts.headers.iter() {
                if header_name != axum::http::header::HOST {
                    //clone header value sang owned type
                    upstream_req = upstream_req.header(header_name, header_value.clone());
                }
            }

            // Chuyển đổi Bytes sang reqwest Body (đã được hỗ trợ sẵn)
            let reqwest_body = reqwest::Body::from(body_bytes);
            let mut upstream_req = upstream_req.body(reqwest_body);

            upstream_req = upstream_req
                .header(
                    &state.config.security.zero_trust.signature_header,
                    &signature_b64,
                )
                .header("X-Gateway-Timestamp", &timestamp);

            let response = match upstream_req.send().await {
                Ok(res) => res,
                Err(e) => {
                    tracing::error!("Lỗi khi gửi request đến upstream: {:?}", e);
                    return (
                        StatusCode::BAD_GATEWAY,
                        format!("Lỗi kết nối upstream: {}", e),
                    )
                        .into_response();
                }
            };

            let mut response_builder = Response::builder().status(response.status());

            // Sao chép ngược các headers nhận từ Upstream về cho Client
            if let Some(headers_mut) = response_builder.headers_mut() {
                *headers_mut = response.headers().clone();
            }

            // Chuyển reqwest response thành Stream và đóng gói vào Axum Body
            let response_body = Body::from_stream(response.bytes_stream());

            match response_builder.body(response_body) {
                Ok(res) => res.into_response(),
                Err(e) => {
                    tracing::error!("Lỗi khi dựng response: {:?}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Lỗi hệ thống khi xây dựng response",
                    )
                        .into_response()
                }
            }
        }
        None => {
            tracing::warn!("Không tìm thấy route khớp cho path: {}", path);
            (
                StatusCode::NOT_FOUND,
                "Không tìm thấy đường dẫn cấu hình tại Gateway",
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        AiNativeConfig, DatabaseConfig, JwtConfig, RouteConfig, SecurityConfig, ServerConfig,
        ZeroTrustConfig,
    };
    use axum::{Router, routing::get};
    use jsonwebtoken::{EncodingKey, Header, encode};
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    // Helper để tạo token JWT hợp lệ cho việc kiểm thử
    fn generate_test_token() -> String {
        let private_key_pem = std::fs::read("certs/jwt_private.pem")
            .expect("Không tìm thấy certs/jwt_private.pem. Vui lòng chạy lệnh sinh khóa trước.");
        let encoding_key = EncodingKey::from_rsa_pem(&private_key_pem).unwrap();

        let exp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize
            + 3600; // Hết hạn sau 1 giờ

        let claims = crate::auth::Claims {
            sub: "test-user-vinh".to_string(),
            exp,
            iss: "test".to_string(),
        };

        encode(
            &Header::new(jsonwebtoken::Algorithm::RS256),
            &claims,
            &encoding_key,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_proxy_latency_and_correctness() {
        // 1. Khởi chạy một Mock Upstream Server trên cổng ngẫu nhiên (cổng 0 sẽ tự chọn cổng trống)
        let mock_upstream =
            Router::new().route("/target-path", get(|| async { "Hello from Upstream" }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = listener.local_addr().unwrap();

        // Chạy Mock Upstream dưới dạng tác vụ chạy ngầm
        tokio::spawn(async move {
            axum::serve(listener, mock_upstream).await.unwrap();
        });

        // 2. Thiết lập cấu hình giả lập trỏ tới Mock Upstream
        let target_url = format!("http://{}", upstream_addr);
        let config = Config {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                log_level: "info".to_string(),
            },
            database: DatabaseConfig {
                redis_url: "redis://127.0.0.1:6379".to_string(),
                connection_timeout: 5000,
            },
            security: SecurityConfig {
                jwt: JwtConfig {
                    secret_key_path: "certs/jwt_public.pem".to_string(),
                    issuer: "test".to_string(),
                },
                zero_trust: ZeroTrustConfig {
                    private_key_path: "certs/gateway_private.pem".to_string(),
                    signature_header: "X-Gateway-Signature".to_string(),
                },
            },
            ai_native: AiNativeConfig {
                model_path: "models/all-MiniLM-L6-v2.onnx".to_string(),
                similarity_threshold: 0.85,
                cache_ttl: 3600,
            },
            routes: vec![RouteConfig {
                path: "/api/test".to_string(),
                target: target_url,
                strip_prefix: true,
                auth_required: false,
                rate_limit: None,
                ai_caching: None,
            }],
        };

        // Đọc public key pem thật để chạy test
        let public_key_pem = std::fs::read("certs/jwt_public.pem")
            .expect("Không tìm thấy certs/jwt_public.pem. Vui lòng chạy lệnh sinh khóa trước.");
        let decoding_key = DecodingKey::from_rsa_pem(&public_key_pem).unwrap();

        // Load Ed25519 signing key cho test
        let signing_key = crate::signature::load_private_key("certs/gateway_private.pk8")
            .await
            .expect(
                "Không tìm thấy certs/gateway_private.pk8. Vui lòng chạy lệnh sinh khóa trước.",
            );

        // 3. Khởi tạo AppState và Router Gateway giả lập
        let state = AppState {
            config: Arc::new(config),
            client: reqwest::Client::new(),
            jwt_decoding_key: Arc::new(decoding_key),
            signing_key: Arc::new(signing_key),
        };

        let app = Router::new().fallback(proxy_handler).with_state(state);

        // 4. Đo độ trễ chuyển tiếp qua Gateway
        use tower::ServiceExt; // Dành cho gọi method oneshot

        // Thực hiện cuộc gọi khởi động (warm-up) để nạp bộ nhớ đệm kết nối
        let req = Request::builder()
            .uri("/api/test/target-path")
            .body(Body::empty())
            .unwrap();
        let _response = app.clone().oneshot(req).await.unwrap();

        let mut total_duration = std::time::Duration::default();
        let iterations = 50000; // Đo trên 50000 request liên tục để có số liệu chính xác và nhanh chóng

        for _ in 0..iterations {
            let req = Request::builder()
                .uri("/api/test/target-path")
                .body(Body::empty())
                .unwrap();

            let start = Instant::now();
            let response = app.clone().oneshot(req).await.unwrap();
            let duration = start.elapsed();

            assert_eq!(response.status(), StatusCode::OK);
            total_duration += duration;
        }

        let avg_latency = total_duration / iterations;
        println!("\n==============================================");
        println!("📊 KẾT QUẢ ĐO ĐỘ TRỄ CHUYỂN TIẾP (LATENCY TEST)");
        println!("- Số lượng request thử nghiệm: {} requests", iterations);
        println!("- Độ trễ trung bình mỗi request: {:?}", avg_latency);
        println!("==============================================\n");

        // Đảm bảo độ trễ chuyển tiếp nội bộ qua Gateway cực kỳ thấp (thường < 5ms trên localhost)
        assert!(
            avg_latency.as_millis() < 50,
            "Độ trễ chuyển tiếp quá lớn: {:?}",
            avg_latency
        );
    }

    #[tokio::test]
    async fn test_jwt_auth_flow() {
        // 1. Khởi chạy một Mock Upstream Server
        let mock_upstream = Router::new().route("/secure-data", get(|| async { "Secret content" }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, mock_upstream).await.unwrap();
        });

        // 2. Thiết lập cấu hình giả lập yêu cầu xác thực JWT (auth_required = true)
        let target_url = format!("http://{}", upstream_addr);
        let config = Config {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                log_level: "info".to_string(),
            },
            database: DatabaseConfig {
                redis_url: "redis://127.0.0.1:6379".to_string(),
                connection_timeout: 5000,
            },
            security: SecurityConfig {
                jwt: JwtConfig {
                    secret_key_path: "certs/jwt_public.pem".to_string(),
                    issuer: "test".to_string(),
                },
                zero_trust: ZeroTrustConfig {
                    private_key_path: "certs/gateway_private.pem".to_string(),
                    signature_header: "X-Gateway-Signature".to_string(),
                },
            },
            ai_native: AiNativeConfig {
                model_path: "models/all-MiniLM-L6-v2.onnx".to_string(),
                similarity_threshold: 0.85,
                cache_ttl: 3600,
            },
            routes: vec![RouteConfig {
                path: "/api/secure".to_string(),
                target: target_url,
                strip_prefix: true,
                auth_required: true, // Yêu cầu xác thực JWT
                rate_limit: None,
                ai_caching: None,
            }],
        };

        let public_key_pem = std::fs::read("certs/jwt_public.pem").unwrap();
        let decoding_key = DecodingKey::from_rsa_pem(&public_key_pem).unwrap();

        // Load Ed25519 signing key cho test
        let signing_key = crate::signature::load_private_key("certs/gateway_private.pk8")
            .await
            .expect("Không tìm thấy certs/gateway_private.pk8");

        let state = AppState {
            config: Arc::new(config),
            client: reqwest::Client::new(),
            jwt_decoding_key: Arc::new(decoding_key),
            signing_key: Arc::new(signing_key),
        };

        let app = Router::new().fallback(proxy_handler).with_state(state);
        use tower::ServiceExt;

        // --- CASE 1: Request không gửi Token -> Bị chặn 401 ---
        let req = Request::builder()
            .uri("/api/secure/secure-data")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // --- CASE 2: Request gửi Token sai/hỏng -> Bị chặn 401 ---
        let req = Request::builder()
            .uri("/api/secure/secure-data")
            .header("Authorization", "Bearer invalid-token-xyz")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // --- CASE 3: Request gửi Token hợp lệ -> Trả về 200 OK và lấy được nội dung ---
        let valid_token = generate_test_token();
        let req = Request::builder()
            .uri("/api/secure/secure-data")
            .header("Authorization", format!("Bearer {}", valid_token))
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Đọc nội dung response
        let body_bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(body_bytes, "Secret content");
    }
}
