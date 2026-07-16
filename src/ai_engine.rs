use std::error::Error;
use tract_onnx::prelude::*;

// Độ dài tối đa của chuỗi input (padding hoặc cắt bớt cho khớp)
// Giá trị 128 là đủ cho hầu hết các câu query ngắn trong API Gateway
const MAX_SEQ_LEN: usize = 128;

// Cấu trúc cốt lõi của Engine, chứa model đã được biên dịch để chạy siêu nhanh
pub struct AiEngine {
    model: SimplePlan<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>,
}

impl AiEngine {
    // Hàm khởi tạo và nạp mô hình từ file .onnx
    // Khai báo input shape cố định [1, MAX_SEQ_LEN] để tract tối ưu đồ thị nhanh chóng
    pub fn new(model_path: &str) -> Result<Self, Box<dyn Error>> {
        let model = onnx()
            .model_for_path(model_path)?
            // Khai báo shape cố định cho 3 input: input_ids, attention_mask, token_type_ids
            // Điều này giúp tract tối ưu đồ thị tính toán trong vài giây thay vì hàng chục phút
            .with_input_fact(0, i64::fact([1, MAX_SEQ_LEN as i64]).into())?
            .with_input_fact(1, i64::fact([1, MAX_SEQ_LEN as i64]).into())?
            .with_input_fact(2, i64::fact([1, MAX_SEQ_LEN as i64]).into())?
            .into_optimized()?
            .into_runnable()?;

        Ok(Self { model })
    }

    // Hàm chuyển đổi Text thành Vector (Embedding)
    pub fn embed(&self, text: &str) -> Result<Vec<f32>, Box<dyn Error>> {
        // Bước 1: Tokenize (đơn giản: lấy mã byte của từng ký tự)
        let raw_tokens: Vec<i64> = text.bytes().map(|b| b as i64).collect();

        // Bước 2: Tính số token thực tế (trước khi pad) để Mean Pooling chỉ tính trên phần thật
        let actual_len = raw_tokens.len().min(MAX_SEQ_LEN);

        // Bước 3: Pad hoặc cắt bớt tokens cho khớp MAX_SEQ_LEN
        let mut padded_tokens = vec![0_i64; MAX_SEQ_LEN];
        padded_tokens[..actual_len].copy_from_slice(&raw_tokens[..actual_len]);

        // Bước 4: Tạo attention_mask: 1 cho token thật, 0 cho padding
        let mut attention_values = vec![0_i64; MAX_SEQ_LEN];
        for i in 0..actual_len {
            attention_values[i] = 1;
        }

        // token_type_ids luôn là 0 (chỉ dùng cho mô hình đa câu, ở đây ta chỉ embed 1 câu)
        let token_type_values = vec![0_i64; MAX_SEQ_LEN];

        // Bước 5: Tạo Tensor với shape cố định [1, MAX_SEQ_LEN]
        let input_ids =
            tract_ndarray::Array2::from_shape_vec((1, MAX_SEQ_LEN), padded_tokens)?;
        let attention_mask =
            tract_ndarray::Array2::from_shape_vec((1, MAX_SEQ_LEN), attention_values)?;
        let token_type_ids =
            tract_ndarray::Array2::from_shape_vec((1, MAX_SEQ_LEN), token_type_values)?;

        let t_value_input: TValue = input_ids.into_tensor().into_tvalue();
        let t_value_attention_mask: TValue = attention_mask.into_tensor().into_tvalue();
        let t_value_token_type_ids: TValue = token_type_ids.into_tensor().into_tvalue();

        // Bước 6: Chạy mô hình
        let result = self.model.run(tvec![
            t_value_input,
            t_value_attention_mask,
            t_value_token_type_ids
        ])?;

        // 5.1. Ép kiểu kết quả (result[0]) về dạng mảng 3D của thư viện ndarray
        let output_tensor = result[0].to_array_view::<f32>()?;
        // Lấy kích thước embedding từ shape thực tế của model
        let embedding_dim = output_tensor.shape()[2];

        // 5.2. Khởi tạo mảng vector kết quả với giá trị 0 (kích thước tự động theo model)
        let mut mean_pooled = vec![0.0_f32; embedding_dim];

        // 5.3. Mean Pooling: Chỉ cộng dồn giá trị của các token THẬT (bỏ qua padding)
        for token_idx in 0..actual_len {
            for dim_idx in 0..embedding_dim {
                mean_pooled[dim_idx] += output_tensor[[0, token_idx, dim_idx]];
            }
        }

        // Chia cho số lượng token THẬT để ra giá trị trung bình
        let actual_len_f32 = actual_len as f32;
        for val in mean_pooled.iter_mut() {
            *val /= actual_len_f32;
        }

        // 5.4. L2 Normalization: Tính độ dài (magnitude) của vector
        // Công thức: Căn bậc 2 của (tổng các bình phương)
        let sum_of_squares: f32 = mean_pooled.iter().map(|&x| x * x).sum();
        let magnitude = sum_of_squares.sqrt();

        // 5.5. Chuẩn hóa vector
        if magnitude > 0.0 {
            for val in mean_pooled.iter_mut() {
                *val /= magnitude;
            }
        }

        // Trả về vector cuối cùng
        Ok(mean_pooled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_engine_embed() {
        // 1. Nạp model ONNX (đường dẫn tương đối từ thư mục project)
        let engine = AiEngine::new("../models/all-MiniLM-L6-v2.onnx")
            .expect("Không thể nạp model ONNX. Hãy kiểm tra file ../models/all-MiniLM-L6-v2.onnx");

        println!("\n==============================================");
        println!("🧠 TEST AI ENGINE - EMBEDDING & SIMILARITY");
        println!("==============================================");

        // 2. Embed 3 câu: 2 câu giống ý nghĩa, 1 câu khác hoàn toàn
        let sentence_a = "What is the weather today?";
        let sentence_b = "How is the weather today?";
        let sentence_c = "I love programming in Rust";

        let vec_a = engine.embed(sentence_a).expect("Lỗi embed câu A");
        let vec_b = engine.embed(sentence_b).expect("Lỗi embed câu B");
        let vec_c = engine.embed(sentence_c).expect("Lỗi embed câu C");

        // 3. In kích thước vector
        println!("\n📐 Kích thước vector:");
        println!("  Câu A: {} chiều", vec_a.len());
        println!("  Câu B: {} chiều", vec_b.len());
        println!("  Câu C: {} chiều", vec_c.len());

        // 4. In 10 phần tử đầu tiên của mỗi vector (để xem dạng dữ liệu)
        println!("\n📊 10 phần tử đầu tiên của mỗi vector:");
        println!("  A: {:?}", &vec_a[..10]);
        println!("  B: {:?}", &vec_b[..10]);
        println!("  C: {:?}", &vec_c[..10]);

        // 5. Tính Cosine Similarity (vì đã chuẩn hóa L2, chỉ cần Dot Product)
        let sim_ab: f32 = vec_a.iter().zip(vec_b.iter()).map(|(a, b)| a * b).sum();
        let sim_ac: f32 = vec_a.iter().zip(vec_c.iter()).map(|(a, c)| a * c).sum();
        let sim_bc: f32 = vec_b.iter().zip(vec_c.iter()).map(|(b, c)| b * c).sum();

        println!("\n🎯 Cosine Similarity (Độ tương đồng):");
        println!("  A vs B (giống nghĩa): {:.4}", sim_ab);
        println!("  A vs C (khác nghĩa):  {:.4}", sim_ac);
        println!("  B vs C (khác nghĩa):  {:.4}", sim_bc);

        // 6. Kiểm tra: Độ tương đồng A-B phải LỚN HƠN A-C
        println!("\n✅ Kiểm tra: sim(A,B) > sim(A,C) => {} > {} => {}",
            sim_ab, sim_ac, sim_ab > sim_ac);
        assert!(sim_ab > sim_ac, "Hai câu giống ý nghĩa phải có độ tương đồng cao hơn!");

        println!("==============================================\n");
    }
}
