//! PDF生成（WASM版）
//!
//! printpdf + js-sys でWASM環境でPDF生成

use photo_ai_common::{AnalysisResult, PdfLayout};

/// PDFを生成してBlobを返す
pub async fn generate_pdf(
    _results: &[AnalysisResult],
    _title: &str,
    _photos_per_page: u8,
) -> Result<Vec<u8>, String> {
    // TODO: printpdf WASMでPDF生成
    // 現時点ではプレースホルダー

    let _layout = PdfLayout::for_photos_per_page(_photos_per_page);

    Err("PDF generation not yet implemented".to_string())
}
