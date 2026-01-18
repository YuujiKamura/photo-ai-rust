//! Excel生成（WASM版）
//!
//! 共通ライブラリのexcel::generate_excel_bufferを使用

use photo_ai_common::excel::{generate_excel_buffer, ImageData};
use photo_ai_common::AnalysisResult;
use base64::{Engine as _, engine::general_purpose::STANDARD};

/// Base64 Data URLから画像データを抽出
fn load_image_from_data_url(data_url: &str) -> Option<ImageData> {
    if !data_url.starts_with("data:image/") {
        return None;
    }

    // data:image/png;base64,xxxxx の形式をパース
    let parts: Vec<&str> = data_url.splitn(2, ',').collect();
    if parts.len() != 2 {
        return None;
    }

    let header = parts[0];
    let base64_data = parts[1];

    // MIMEタイプから拡張子を取得
    let extension = if header.contains("image/png") {
        "png"
    } else if header.contains("image/gif") {
        "gif"
    } else if header.contains("image/webp") {
        "webp"
    } else {
        "jpeg"
    };

    // Base64デコード
    let data = STANDARD.decode(base64_data).ok()?;

    Some(ImageData {
        data,
        extension: extension.to_string(),
    })
}

/// Excelを生成してバイト配列を返す
pub async fn generate_excel(
    results: &[AnalysisResult],
    _title: &str,
    photos_per_page: u8,
) -> Result<Vec<u8>, String> {
    // file_pathにはData URLが入っている想定
    generate_excel_buffer(results, photos_per_page, load_image_from_data_url)
}
