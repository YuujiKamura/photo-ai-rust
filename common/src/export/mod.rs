//! Export core modules shared across CLI and WASM wrappers.

use thiserror::Error;

#[cfg(feature = "pdf")]
pub mod pdf;

#[cfg(all(feature = "pdf", not(target_arch = "wasm32")))]
pub mod pdf_embed;

#[cfg(feature = "excel")]
pub mod excel;

/// エクスポート関連エラー（PDF/Excel）
#[derive(Error, Debug)]
pub enum ExportError {
    #[error("Export failed ({format}): {detail}")]
    Failed { format: String, detail: String },
}

/// Common trait for export backends.
pub trait Exporter {
    fn export(
        &self,
        results: &[crate::types::AnalysisResult],
        image_loader: &dyn Fn(&str) -> Option<ImageData>,
    ) -> crate::error::Result<Vec<u8>>;
    fn format_name(&self) -> &str;
}

/// 画像データ（バイト配列）- Excel/PDF共通
pub struct ImageData {
    pub data: Vec<u8>,
    pub extension: String,  // "png", "jpeg", "gif"
}
