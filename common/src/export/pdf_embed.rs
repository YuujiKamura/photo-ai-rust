//! PDF解析結果埋め込みモジュール（非WASM専用）
//!
//! pdf-analysis-embed ライブラリを使用して、PDFのInfo辞書に
//! 解析結果をJSON形式で埋め込みます。

use std::path::Path;
use crate::types::AnalysisResult;

/// PDF生成後に解析結果を埋め込む
///
/// pdf-analysis-embed ライブラリを使用して、PDFのInfo辞書に
/// 解析結果をJSON形式で埋め込みます。これにより再解析をスキップできます。
///
/// # Arguments
/// * `pdf_path` - 生成されたPDFファイルのパス
/// * `results` - 埋め込む解析結果
///
/// # Example
/// ```ignore
/// use photo_ai_common::export::pdf_embed::embed_analysis_to_pdf;
/// embed_analysis_to_pdf(Path::new("output.pdf"), &results)?;
/// ```
pub fn embed_analysis_to_pdf(pdf_path: &Path, results: &[AnalysisResult]) -> crate::error::Result<()> {
    use pdf_analysis_embed::{embed_data, EmbedConfig, EmbeddedData};

    // 解析結果をJSONにシリアライズ
    let json_content = serde_json::to_string(results)?;

    // EmbeddedDataを構築
    let data = EmbeddedData::new(json_content)
        .with_source("photo-ai-rust")
        .with_extra(format!("photos: {}", results.len()));

    // photo-ai-rust用のプレフィックスで埋め込み
    let config = EmbedConfig::with_prefix("PhotoAiRust");

    embed_data(pdf_path, &data, &config)
        .map_err(|e| super::ExportError::Failed {
            format: "PDF".to_string(),
            detail: format!("埋め込みに失敗: {}", e),
        })?;

    Ok(())
}
