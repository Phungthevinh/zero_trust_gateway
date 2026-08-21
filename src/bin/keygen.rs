// =====================================================================
// Project: Zero-Trust API Gateway
// Author: Phung The Vinh (ptvstar2003@gmail.com)
// Copyright © 2026. All rights reserved.
// =====================================================================

use ring::rand::SystemRandom;
use ring::signature::Ed25519KeyPair;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    //Khởi tạo trình tạo số ngẫu nhiên an toàn của hệ điều hành
    let rng = SystemRandom::new();

    //Sinh khóa PKCS#8 chuẩn Ed25519
    let pkcs8_document = Ed25519KeyPair::generate_pkcs8(&rng).expect("Lỗi khi sinh khóa Ed25519");

    //kiểm tra thư mục certs có tồn tại hay chưa
    if !Path::new("certs").exists() {
        match fs::create_dir("certs") {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("Không thể tạo thư mục certs: {}", e);
                return;
            }
        }
    }

    //Ghi khóa Private ra file (định dạng PKCS#8, thường dùng đuôi .pk8)
    let private_key_path = "certs/gateway_private.pk8";

    //tạo file gateway_private.pk8
    let mut file = match File::create(private_key_path) {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("Không thể tạo file gateway_private.pk8: {}", e);
            return;
        }
    };
    file.write_all(pkcs8_document.as_ref())
        .expect("lỗi khi ghi nội dung khóa ra file");

    println!(
        "đã sinh thành công khóa và đang lưu tại: {}",
        private_key_path
    );
}
