//! PDF解析結果埋め込みモジュール（非WASM専用）
//!
//! pdf-analysis-embed ライブラリを使用して、PDFのInfo辞書に
//! 解析結果をJSON形式で埋め込みます。

use std::path::Path;
use crate::types::AnalysisResult;
use pdf_analysis_embed::{EmbedConfig, EmbeddedData};

/// PDFプレフィックス定数
const PDF_PREFIX: &str = "PhotoAiRust";

pub fn embed_analysis_to_pdf(pdf_path: &Path, results: &[AnalysisResult]) -> crate::error::Result<()> {
    use pdf_analysis_embed::embed_data;

    // 解析結果をJSONにシリアライズ
    let json_content = serde_json::to_string(results)?;

    // EmbeddedDataを構築
    let data = EmbeddedData::new(json_content)
        .with_source("photo-ai-rust")
        .with_extra(format!("photos: {}", results.len()));

    // photo-ai-rust用のプレフィックスで埋め込み
    let config = EmbedConfig::with_prefix(PDF_PREFIX);

    embed_data(pdf_path, &data, &config)
        .map_err(|e| super::ExportError::Failed {
            format: "PDF".to_string(),
            detail: format!("埋め込みに失敗: {}", e),
        })?;

    Ok(())
}

/// PDFのInfo辞書から解析結果を読み取る
pub fn read_analysis_from_pdf(pdf_path: &Path) -> crate::error::Result<Vec<AnalysisResult>> {
    let config = EmbedConfig::with_prefix(PDF_PREFIX);
    let data = pdf_analysis_embed::read_data(pdf_path, &config)
        .map_err(|e| super::ExportError::Failed {
            format: "PDF".to_string(),
            detail: format!("埋め込みデータ読み取り失敗: {}", e),
        })?;
    let data = data.ok_or_else(|| super::ExportError::Failed {
        format: "PDF".to_string(),
        detail: "埋め込みデータが見つかりません".to_string(),
    })?;
    let results: Vec<AnalysisResult> = serde_json::from_str(&data.content)?;
    Ok(results)
}
