//! Excel生成（WASM版）
//!
//! JavaScript Bridge経由でExcelJSを使用してExcel生成

use crate::export::js_bindings::{generate_excel_js, JsPhotoEntry};
use photo_ai_common::AnalysisResult;

/// Excelを生成してバイト配列を返す
pub async fn generate_excel(
    results: &[AnalysisResult],
    title: &str,
    photos_per_page: u8,
) -> Result<Vec<u8>, String> {
    // AnalysisResult → JsPhotoEntry 変換
    let photos: Vec<JsPhotoEntry> = results.iter()
        .map(JsPhotoEntry::from)
        .collect();

    let photos_json = serde_json::to_string(&photos)
        .map_err(|e| format!("JSON serialization failed: {}", e))?;

    let options_json = serde_json::to_string(&serde_json::json!({
        "title": title,
        "photosPerPage": photos_per_page
    })).map_err(|e| format!("Options serialization failed: {}", e))?;

    // JavaScript呼び出し
    let result = generate_excel_js(&photos_json, &options_json)
        .await
        .map_err(|e| format!("Excel generation failed: {:?}", e))?;

    // JsValue → Vec<u8> 変換
    let array = js_sys::Uint8Array::new(&result);
    Ok(array.to_vec())
}
