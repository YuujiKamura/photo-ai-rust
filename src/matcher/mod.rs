use crate::analyzer::AnalysisResult;
use crate::error::{PhotoAiError, Result};
use std::path::Path;

pub fn match_with_master(
    results: &[AnalysisResult],
    master_path: &Path,
) -> Result<Vec<AnalysisResult>> {
    if !master_path.exists() {
        return Err(PhotoAiError::FileNotFound(master_path.display().to_string()));
    }

    let content = std::fs::read_to_string(master_path)?;
    let _master: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| PhotoAiError::InvalidMaster(format!("JSONパースエラー: {}", e)))?;

    // TODO: マスタ照合ロジック実装
    // 現時点では結果をそのまま返す
    Ok(results.to_vec())
}
