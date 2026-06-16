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
