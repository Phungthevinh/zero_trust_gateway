use axum::http::HeaderMap;
use jsonwebtoken::{Algorithm, DecodingKey, TokenData, Validation, decode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // User ID hoặc Username
    pub exp: usize,  // Hạn hết hạn của token (Epoch timestamp)
    pub iss: String, // Nhà phát hành token (Issuer)
                     // Bạn có thể thêm các trường khác như role, email... nếu muốn
}

pub fn verify_token(
    token: &str,
    decoding_key: &DecodingKey,
    expected_issuer: &str,
) -> Result<TokenData<Claims>, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::RS256);

    validation.set_issuer(&[expected_issuer]);
    decode::<Claims>(token, decoding_key, &validation)
}

pub fn extract_token_from_header(header: &HeaderMap) -> Option<&str> {
    if let Some(auth_header) = header.get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            let mut parts = auth_str.split_whitespace();
            if parts.next() == Some("Bearer") {
                if let Some(token) = parts.next() {
                    if parts.next().is_none() {
                        return Some(token);
                    }
                }
            }
        }
    }
    None
}
