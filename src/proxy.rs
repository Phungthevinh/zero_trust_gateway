use crate::config::Config;
use crate::signature;
use crate::{auth, fast_reject};
use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{HeaderMap, Request, Response, StatusCode},
    response::IntoResponse,
};
use jsonwebtoken::DecodingKey;
use ring::signature::Ed25519KeyPair;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::metrics::GatewayMetrics;
use crate::rate_limit;
use crate::semantic_cache::SemanticCache;

// Định nghĩa AppState chia sẻ dữ liệu giữa các luồng
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub client: reqwest::Client,
    pub jwt_decoding_key: Arc<DecodingKey>,
    pub signing_key: Arc<Ed25519KeyPair>,
    pub rate_limiter: Arc<rate_limit::RateLimiter>,
    pub fast_reject: Arc<fast_reject::FastRejectFilter>,
    pub semantic_cache: Option<Arc<SemanticCache>>,
    pub metrics: Arc<GatewayMetrics>,
}

// Handler nhận request và khớp route
pub async fn proxy_handler(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
) -> impl IntoResponse {
    let _guard = state.metrics.record_active_request();
    let (parts, body) = req.into_parts();

    let path = parts.uri.path();

    state.metrics.total_requests.fetch_add(1, Ordering::Relaxed);

    // 1. Fast Reject Check
    if let Err(e) = state.fast_reject.check_request(&parts) {
        tracing::warn!("Fast reject: {}", e);
        state.metrics.total_errors.fetch_add(1, Ordering::Relaxed);
        return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
    }

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

            // 1. Rate Limiting Check
            let ip_key =
                get_client_ip(addr, &parts.headers, &state.config.security.trusted_proxies);
            if !state.rate_limiter.check_request(&ip_key).await {
                state.metrics.total_errors.fetch_add(1, Ordering::Relaxed);
                tracing::warn!("Rate limit exceeded for IP: {}", ip_key);
                return (StatusCode::TOO_MANY_REQUESTS, "Too Many Requests").into_response();
            }

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
                                state.metrics.total_errors.fetch_add(1, Ordering::Relaxed);
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
                    state.metrics.total_errors.fetch_add(1, Ordering::Relaxed);
                    return (StatusCode::BAD_REQUEST, "Không thể đọc request body").into_response();
                }
            };

            if route.ai_caching == Some(true) && state.semantic_cache.is_some() {
                return handle_ai_request(state, parts, body_bytes, target_url)
                    .await
                    .into_response();
            }

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
                    state.metrics.total_errors.fetch_add(1, Ordering::Relaxed);
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
                    state.metrics.total_errors.fetch_add(1, Ordering::Relaxed);
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
            state.metrics.total_errors.fetch_add(1, Ordering::Relaxed);
            tracing::warn!("Không tìm thấy route khớp cho path: {}", path);
            (
                StatusCode::NOT_FOUND,
                "Không tìm thấy đường dẫn cấu hình tại Gateway",
            )
                .into_response()
        }
    }
}

async fn handle_ai_request(
    state: AppState,
    parts: axum::http::request::Parts,
    body_bytes: axum::body::Bytes,
    target_url: String,
) -> Response<Body> {
    let json_body: serde_json::Value = match serde_json::from_slice(&body_bytes) {
        Ok(val) => val,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid JSON body").into_response(),
    };

    let is_dynamic = is_mcp_or_tool_request(&json_body);
    let mut extracted_prompt = None;

    if !is_dynamic {
        let prompt = json_body
            .get("messages")
            .and_then(|m| m.as_array())
            .and_then(|arr| {
                arr.iter()
                    .rev()
                    .find(|msg| msg.get("role").and_then(|r| r.as_str()) == Some("user"))
            })
            .and_then(|msg| msg.get("content").and_then(|c| c.as_str()));
        let prompt = match prompt {
            Some(p) => p,
            None => {
                return (StatusCode::BAD_REQUEST, "Missing user prompt in messages")
                    .into_response();
            }
        };

        extracted_prompt = Some(prompt.clone());
        let cache = state.semantic_cache.as_ref().unwrap();

        if let Some(cached_response) = cache.lookup(&prompt) {
            tracing::info!("Semantic Cache HIT cho prompt: {}", prompt);

            state.metrics.ai_cache_hits.fetch_add(1, Ordering::Relaxed);

            return Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("X-Cache", "HIT")
                .body(Body::from(cached_response))
                .unwrap();
        }

        state
            .metrics
            .ai_cache_misses
            .fetch_add(1, Ordering::Relaxed);
    }

    let mut upstream_req = state.client.request(parts.method.clone(), &target_url);
    // 2. Lặp qua tất cả Headers của Client gửi lên, copy sang Request mới
    for (header_name, header_value) in parts.headers.iter() {
        if header_name != axum::http::header::HOST {
            // Bỏ qua Host vì reqwest sẽ tự điền Host mới
            upstream_req = upstream_req.header(header_name, header_value.clone());
        }
    }

    // 3. Đính kèm nguyên cái Body gốc vào
    let reqwest_body = reqwest::Body::from(body_bytes.clone()); // body_bytes đã có sẵn ở tham số hàm
    upstream_req = upstream_req.body(reqwest_body);

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

    let status = response.status();
    let res_bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!("Lỗi khi đọc response từ upstream: {:?}", e);
            return (
                StatusCode::BAD_GATEWAY,
                format!("Lỗi đọc response từ upstream: {}", e),
            )
                .into_response();
        }
    };

    if status.is_success() && !is_dynamic {
        if let Some(prompt) = extracted_prompt {
            // Khôi phục logic parse JSON cũ ở đây
            if let Ok(res_json) = serde_json::from_slice::<serde_json::Value>(&res_bytes) {
                if let Some(ai_text) = res_json["choices"][0]["message"]["content"].as_str() {
                    let cache = state.semantic_cache.as_ref().unwrap();
                    cache.insert(prompt, ai_text.to_string());
                    tracing::info!(
                        "Semantic Cache MISS, response cached cho prompt: {}",
                        prompt
                    );
                }
            }
        }
    }

    // Đặt ở cuối hàm
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .header("X-Cache", "MISS")
        .body(Body::from(res_bytes))
        .unwrap_or_else(|err| {
            tracing::error!("Lỗi khi dựng response: {:?}", err);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Internal Server Error"))
                .unwrap()
        })
}

fn is_mcp_or_tool_request(json_body: &serde_json::Value) -> bool {
    let tool = json_body.get("tools").map_or(false, |v| v.is_array());

    let is_jsonrpc = json_body
        .get("jsonrpc")
        .and_then(|v| v.as_str())
        .map_or(false, |v| v == "2.0")
        && json_body.get("method").is_some();
    tool || is_jsonrpc
}

fn get_client_ip(
    IpAddr: SocketAddr,
    headers: &HeaderMap,
    trusted_proxies: &Vec<ipnetwork::IpNetwork>,
) -> String {
    //trích xuất IPAddr từ soketAddr
    let tcp_ip = IpAddr.ip();
    if trusted_proxies
        .iter()
        .any(|network| network.contains(tcp_ip))
    {
        let forwarded = headers
            .get("X-Forwarded-For")
            .and_then(|value| value.to_str().ok())
            .and_then(|s| s.split(',').next())
            .ok_or_else(|| "Không tìm thấy header X-Forwarded-For");

        return match forwarded {
            Ok(ip) => ip.to_string(),
            Err(_) => tcp_ip.to_string(),
        };
    }

    tcp_ip.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        AiNativeConfig, DatabaseConfig, FastRejectConfig, JwtConfig, RouteConfig, SecurityConfig,
        ServerConfig, ZeroTrustConfig,
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
                fast_reject: FastRejectConfig {
                    max_header_count: 50,
                    max_uri_length: 2048,
                    max_body_size: 10 * 1024 * 1024,
                    blocked_paths: vec![],
                    ip_blacklist: vec![],
                },
                trusted_proxies: vec!["192.168.0.0/16".parse().unwrap()],
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
        let rate_limiter = Arc::new(crate::rate_limit::RateLimiter::new(100000.0, 100000.0, 1));
        let fast_reject = Arc::new(crate::fast_reject::FastRejectFilter::new(&config));
        let state = AppState {
            config: Arc::new(config),
            client: reqwest::Client::new(),
            jwt_decoding_key: Arc::new(decoding_key),
            signing_key: Arc::new(signing_key),
            rate_limiter,
            fast_reject,
            semantic_cache: None,
            metrics: Arc::new(crate::metrics::GatewayMetrics::default()),
        };

        let app = Router::new().fallback(proxy_handler).with_state(state);

        // 4. Đo độ trễ chuyển tiếp qua Gateway
        use tower::ServiceExt; // Dành cho gọi method oneshot

        // Thực hiện cuộc gọi khởi động (warm-up) để nạp bộ nhớ đệm kết nối
        let req = Request::builder()
            .uri("/api/test/target-path")
            .header("Host", "localhost")
            .extension(axum::extract::ConnectInfo(
                "127.0.0.1:8080".parse::<std::net::SocketAddr>().unwrap(),
            ))
            .body(Body::empty())
            .unwrap();
        let _response = app.clone().oneshot(req).await.unwrap();

        let mut total_duration = std::time::Duration::default();
        let iterations = 50000; // Đo trên 50000 request liên tục để có số liệu chính xác và nhanh chóng

        for _ in 0..iterations {
            let req = Request::builder()
                .uri("/api/test/target-path")
                .header("Host", "localhost")
                .extension(axum::extract::ConnectInfo(
                    "127.0.0.1:8080".parse::<std::net::SocketAddr>().unwrap(),
                ))
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
                fast_reject: FastRejectConfig {
                    max_header_count: 50,
                    max_uri_length: 2048,
                    max_body_size: 10 * 1024 * 1024,
                    blocked_paths: vec![],
                    ip_blacklist: vec![],
                },
                trusted_proxies: vec!["192.168.0.0/16".parse().unwrap()],
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

        let rate_limiter = Arc::new(crate::rate_limit::RateLimiter::new(100.0, 10.0, 1));
        let fast_reject = Arc::new(crate::fast_reject::FastRejectFilter::new(&config));
        let state = AppState {
            config: Arc::new(config),
            client: reqwest::Client::new(),
            jwt_decoding_key: Arc::new(decoding_key),
            signing_key: Arc::new(signing_key),
            rate_limiter,
            fast_reject,
            semantic_cache: None,
            metrics: Arc::new(crate::metrics::GatewayMetrics::default()),
        };

        let app = Router::new().fallback(proxy_handler).with_state(state);
        use tower::ServiceExt;

        // --- CASE 1: Request không gửi Token -> Bị chặn 401 ---
        let req = Request::builder()
            .uri("/api/secure/secure-data")
            .header("Host", "localhost")
            .extension(axum::extract::ConnectInfo(
                "127.0.0.1:8080".parse::<std::net::SocketAddr>().unwrap(),
            ))
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // --- CASE 2: Request gửi Token sai/hỏng -> Bị chặn 401 ---
        let req = Request::builder()
            .uri("/api/secure/secure-data")
            .header("Host", "localhost")
            .extension(axum::extract::ConnectInfo(
                "127.0.0.1:8080".parse::<std::net::SocketAddr>().unwrap(),
            ))
            .header("Authorization", "Bearer invalid-token-xyz")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // --- CASE 3: Request gửi Token hợp lệ -> Trả về 200 OK và lấy được nội dung ---
        let valid_token = generate_test_token();
        let req = Request::builder()
            .uri("/api/secure/secure-data")
            .header("Host", "localhost")
            .extension(axum::extract::ConnectInfo(
                "127.0.0.1:8080".parse::<std::net::SocketAddr>().unwrap(),
            ))
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

    #[tokio::test]
    async fn test_ai_caching_flow() {
        // 1. Khởi chạy Mock Upstream Server giả lập OpenAI
        let mock_openai = Router::new().route(
            "/v1/chat/completions",
            axum::routing::post(|| async {
                let mock_response = serde_json::json!({
                    "id": "chatcmpl-123",
                    "object": "chat.completion",
                    "created": 1677652288,
                    "model": "gpt-4o",
                    "choices": [{
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": "Đây là câu trả lời từ Upstream AI!"
                        },
                        "finish_reason": "stop"
                    }],
                    "usage": {
                        "prompt_tokens": 9,
                        "completion_tokens": 12,
                        "total_tokens": 21
                    }
                });
                axum::Json(mock_response)
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, mock_openai).await.unwrap();
        });

        // 2. Thiết lập cấu hình Gateway trỏ tới Mock OpenAI
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
                fast_reject: FastRejectConfig {
                    max_header_count: 50,
                    max_uri_length: 2048,
                    max_body_size: 10 * 1024 * 1024,
                    blocked_paths: vec![],
                    ip_blacklist: vec![],
                },
                trusted_proxies: vec!["192.168.0.0/16".parse().unwrap()],
            },
            ai_native: AiNativeConfig {
                model_path: "models/all-MiniLM-L6-v2.onnx".to_string(),
                similarity_threshold: 0.95,
                cache_ttl: 3600,
            },
            routes: vec![RouteConfig {
                path: "/api/ai".to_string(),
                target: target_url,
                strip_prefix: true,
                auth_required: false,
                rate_limit: None,
                ai_caching: Some(true),
            }],
        };

        // 3. Khởi tạo Semantic Cache
        let engine = crate::ai_engine::AiEngine::new("../models/all-MiniLM-L6-v2.onnx")
            .expect("Không thể nạp model ONNX.");
        let ai_engine = Arc::new(engine);
        let semantic_cache = Arc::new(SemanticCache::new(
            ai_engine,
            config.ai_native.similarity_threshold,
            config.ai_native.cache_ttl,
        ));

        let public_key_pem = std::fs::read("certs/jwt_public.pem").unwrap();
        let decoding_key = DecodingKey::from_rsa_pem(&public_key_pem).unwrap();
        let signing_key = crate::signature::load_private_key("certs/gateway_private.pk8")
            .await
            .expect("Không tìm thấy certs/gateway_private.pk8");
        let rate_limiter = Arc::new(crate::rate_limit::RateLimiter::new(100.0, 10.0, 1));
        let fast_reject = Arc::new(crate::fast_reject::FastRejectFilter::new(&config));

        let state = AppState {
            config: Arc::new(config),
            client: reqwest::Client::new(),
            jwt_decoding_key: Arc::new(decoding_key),
            signing_key: Arc::new(signing_key),
            rate_limiter,
            fast_reject,
            semantic_cache: Some(semantic_cache),
            metrics: Arc::new(crate::metrics::GatewayMetrics::default()),
        };

        let app = Router::new().fallback(proxy_handler).with_state(state);
        use tower::ServiceExt;

        let payload = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Xin chào AI!"}]
        });

        // --- Lần 1: Cache MISS (Gọi Upstream) ---
        let req1 = Request::builder()
            .method("POST")
            .uri("/api/ai/v1/chat/completions")
            .header("Host", "localhost")
            .extension(axum::extract::ConnectInfo(
                "127.0.0.1:8080".parse::<std::net::SocketAddr>().unwrap(),
            ))
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();

        let start1 = std::time::Instant::now();
        let res1 = app.clone().oneshot(req1).await.unwrap();
        let duration1 = start1.elapsed();

        assert_eq!(res1.status(), StatusCode::OK);
        assert_eq!(res1.headers().get("X-Cache").unwrap(), "MISS");

        let body_bytes1 = axum::body::to_bytes(res1.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let res1_json: serde_json::Value = serde_json::from_slice(&body_bytes1).unwrap();
        assert_eq!(
            res1_json["choices"][0]["message"]["content"],
            "Đây là câu trả lời từ Upstream AI!"
        );

        // --- Lần 2: Cache HIT (Cùng câu hỏi -> Đọc từ Semantic Cache) ---
        let payload2 = serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "AI ơi, chào bạn!"}] // Câu hỏi tương đồng
        });

        let req2 = Request::builder()
            .method("POST")
            .uri("/api/ai/v1/chat/completions")
            .header("Host", "localhost")
            .extension(axum::extract::ConnectInfo(
                "127.0.0.1:8080".parse::<std::net::SocketAddr>().unwrap(),
            ))
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&payload2).unwrap()))
            .unwrap();

        let start2 = std::time::Instant::now();
        let res2 = app.clone().oneshot(req2).await.unwrap();
        let duration2 = start2.elapsed();

        assert_eq!(res2.status(), StatusCode::OK);
        assert_eq!(res2.headers().get("X-Cache").unwrap(), "HIT");

        let body_bytes2 = axum::body::to_bytes(res2.into_body(), 1024 * 1024)
            .await
            .unwrap();
        // Cấu trúc trả về từ HIT Cache hiện tại là Raw String từ Cache, hoặc bạn đã bọc nó lại.
        // CacheHIT lúc trước trả về string gốc
        let hit_text = String::from_utf8(body_bytes2.to_vec()).unwrap();
        assert_eq!(hit_text, "Đây là câu trả lời từ Upstream AI!");

        println!("\n==============================================");
        println!("🚀 KẾT QUẢ ĐO LƯỜNG SEMANTIC CACHE");
        println!("- Lần 1 (Cache MISS -> Upstream): {:?}", duration1);
        println!("- Lần 2 (Cache HIT -> Local): {:?}", duration2);
        println!("==============================================\n");
    }

    #[tokio::test]
    async fn test_admin_metrics_endpoint() {
        use tower::ServiceExt;

        // 1. Khởi tạo cấu hình giả lập tối thiểu
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
                fast_reject: FastRejectConfig {
                    max_header_count: 50,
                    max_uri_length: 2048,
                    max_body_size: 10 * 1024 * 1024,
                    blocked_paths: vec![],
                    ip_blacklist: vec![],
                },
                trusted_proxies: vec!["192.168.0.0/16".parse().unwrap()],
            },
            ai_native: AiNativeConfig {
                model_path: "models/all-MiniLM-L6-v2.onnx".to_string(),
                similarity_threshold: 0.95,
                cache_ttl: 3600,
            },
            routes: vec![],
        };

        // 2. Khởi tạo các dependencies cho AppState
        let public_key_pem = std::fs::read("certs/jwt_public.pem").unwrap();
        let decoding_key = DecodingKey::from_rsa_pem(&public_key_pem).unwrap();
        let signing_key = crate::signature::load_private_key("certs/gateway_private.pk8")
            .await
            .expect("Không tìm thấy certs/gateway_private.pk8");
        let rate_limiter = Arc::new(crate::rate_limit::RateLimiter::new(100.0, 10.0, 1));
        let fast_reject = Arc::new(crate::fast_reject::FastRejectFilter::new(&config));
        let metrics = Arc::new(crate::metrics::GatewayMetrics::default());

        // Giả lập tăng metric
        metrics
            .total_requests
            .fetch_add(42, std::sync::atomic::Ordering::Relaxed);
        metrics
            .ai_cache_hits
            .fetch_add(10, std::sync::atomic::Ordering::Relaxed);
        metrics
            .ai_cache_misses
            .fetch_add(5, std::sync::atomic::Ordering::Relaxed);

        let state = AppState {
            config: Arc::new(config),
            client: reqwest::Client::new(),
            jwt_decoding_key: Arc::new(decoding_key),
            signing_key: Arc::new(signing_key),
            rate_limiter,
            fast_reject,
            semantic_cache: None,
            metrics,
        };

        // 3. Dựng Router gắn handler admin_metrics
        let app = Router::new()
            .route("/admin/metrics", get(crate::metrics::admin_metrics_handler))
            .with_state(state);

        // 4. Tạo Request GET /admin/metrics
        let req = Request::builder()
            .method("GET")
            .uri("/admin/metrics")
            .body(Body::empty())
            .unwrap();

        // 5. Gửi request vào router
        let res = app.oneshot(req).await.unwrap();

        // 6. Kiểm tra kết quả trả về
        assert_eq!(res.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(json["total_requests"], 42);
        assert_eq!(json["ai_cache_hits"], 10);
        assert_eq!(json["ai_cache_misses"], 5);
        assert_eq!(json["total_errors"], 0);

        println!("\n✅ Admin Metrics Endpoint Test PASSED: {:?}", json);
    }
}
