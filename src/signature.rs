// =====================================================================
// Project: Zero-Trust API Gateway
// Author: Phung The Vinh (ptvstar2003@gmail.com)
// Copyright © 2026. All rights reserved.
// =====================================================================

use base64::prelude::*;
use chrono::Utc;
use ring::digest;
use ring::signature::Ed25519KeyPair;
use std::fs;
use std::path::Path;

pub async fn load_private_key(path: &str) -> Result<Ed25519KeyPair, String> {
    let pth = Path::new(path);
    match fs::read(pth) {
        Ok(bytes) => match Ed25519KeyPair::from_pkcs8_maybe_unchecked(&bytes) {
            Ok(key) => Ok(key),
            Err(e) => Err(e.to_string()),
        },
        Err(e) => Err(e.to_string()),
    }
}

pub async fn sign_request(
    key_pair: &Ed25519KeyPair,
    method: &str,
    path: &str,
    body_bytes: &[u8],
) -> (String, String) {
    let timestamp = Utc::now().to_rfc3339();

    let body_hash = digest::digest(&digest::SHA256, body_bytes);

    let body_hash_hex: String = body_hash
        .as_ref()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();

    let msg = format!("{}:{}:{}:{}", method, path, timestamp, body_hash_hex);

    //KÝ bằng Ed25519
    let signature = key_pair.sign(msg.as_bytes());
    let signature_bytes = signature.as_ref();

    let signature_b64 = BASE64_STANDARD.encode(signature_bytes);

    (signature_b64, timestamp)
}
