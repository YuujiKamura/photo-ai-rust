//! PDF生成（WASM版）
//!
//! JavaScript Bridge経由でpdf-libを使用してPDF生成

use crate::export::js_bindings::{generate_pdf_js, JsLayoutConfig, JsPhotoEntry};
use photo_ai_common::{AnalysisResult, PdfLayout};

/// PDFを生成してバイト配列を返す
pub async fn generate_pdf(
    results: &[AnalysisResult],
    title: &str,
    photos_per_page: u8,
) -> Result<Vec<u8>, String> {
    let layout = PdfLayout::for_photos_per_page(photos_per_page);

    // AnalysisResult → JsPhotoEntry 変換
    let photos: Vec<JsPhotoEntry> = results.iter().map(JsPhotoEntry::from).collect();

    let photos_json =
        serde_json::to_string(&photos).map_err(|e| format!("JSON serialization failed: {}", e))?;

    let layout_config = JsLayoutConfig::from(&layout);
    let layout_json = serde_json::to_string(&layout_config)
        .map_err(|e| format!("Layout serialization failed: {}", e))?;

    let options_json = serde_json::to_string(&serde_json::json!({
        "title": title
    }))
    .map_err(|e| format!("Options serialization failed: {}", e))?;

    // JavaScript呼び出し
    let result = generate_pdf_js(&photos_json, &layout_json, &options_json)
        .await
        .map_err(|e| format!("PDF generation failed: {:?}", e))?;

    // JsValue → Vec<u8> 変換
    let array = js_sys::Uint8Array::new(&result);
    Ok(array.to_vec())
}
