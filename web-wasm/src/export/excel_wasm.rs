//! Excel生成（WASM版）
//!
//! rust_xlsxwriter + wasm でWASM環境でExcel生成

use photo_ai_common::{AnalysisResult, ExcelLayout};

/// Excelを生成してバイト列を返す
pub async fn generate_excel(
    _results: &[AnalysisResult],
    _title: &str,
    _photos_per_page: u8,
) -> Result<Vec<u8>, String> {
    // TODO: rust_xlsxwriter WASMでExcel生成
    // 現時点ではプレースホルダー

    let _layout = ExcelLayout::for_photos_per_page(_photos_per_page);

    Err("Excel generation not yet implemented".to_string())
}
