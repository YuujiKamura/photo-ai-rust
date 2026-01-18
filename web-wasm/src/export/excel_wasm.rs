//! Excel生成（WASM版）
//!
//! JavaScript Bridge経由でexceljsを使用してExcel生成

use crate::export::js_bindings::{generate_excel_js, photos_to_json};
use photo_ai_common::AnalysisResult;

/// Excelを生成してバイト配列を返す
pub async fn generate_excel(
    results: &[AnalysisResult],
    title: &str,
    photos_per_page: u8,
) -> Result<Vec<u8>, String> {
    let photos_json = photos_to_json(results)?;
    let options_json = serde_json::to_string(&serde_json::json!({
        "title": title,
        "photosPerPage": photos_per_page,
    }))
    .map_err(|e| format!("Options serialization failed: {}", e))?;

    let result = generate_excel_js(&photos_json, &options_json)
        .await
        .map_err(|e| format!("Excel generation failed: {:?}", e))?;

    let array = js_sys::Uint8Array::new(&result);
    Ok(array.to_vec())
}
