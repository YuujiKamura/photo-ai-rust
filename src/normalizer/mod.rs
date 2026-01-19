//! 後解析（正規化）モジュール
//!
//! 個別画像解析後に、グループ単位で計測値を統一する。
//!
//! ## 処理フロー（予定）
//! - 温度管理: 3枚単位（全景+ボードアップ+温度計アップ）で統一
//! - 出来形管理: 同一測点のセット全体で統一

pub mod measurements;

use crate::analyzer::AnalysisResult;

/// 正規化結果
#[derive(Debug, Clone)]
pub struct NormalizationResult {
    /// 修正内容のリスト
    pub corrections: Vec<NormalizationCorrection>,
    /// 統計情報
    pub stats: NormalizationStats,
}

/// 個別の修正内容
#[derive(Debug, Clone)]
pub struct NormalizationCorrection {
    /// ファイル名
    pub file_name: String,
    /// 修正対象フィールド
    pub field: CorrectionField,
    /// 修正前の値
    pub original: String,
    /// 修正後の値
    pub corrected: String,
    /// 修正理由
    pub reason: String,
}

/// 修正対象フィールド
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrectionField {
    Measurements,
}

impl std::fmt::Display for CorrectionField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CorrectionField::Measurements => write!(f, "計測値"),
        }
    }
}

/// 正規化の統計情報
#[derive(Debug, Clone, Default)]
pub struct NormalizationStats {
    /// 処理したレコード数
    pub total_records: usize,
    /// 修正したレコード数
    pub corrected_records: usize,
    /// 計測値の修正数
    pub measurement_corrections: usize,
}

/// 正規化オプション
#[derive(Debug, Clone)]
pub struct NormalizationOptions {
    /// 計測値グループ統一を有効にする
    pub unify_measurements: bool,
}

impl Default for NormalizationOptions {
    fn default() -> Self {
        Self {
            unify_measurements: true,
        }
    }
}

/// 解析結果を正規化する
///
/// # Arguments
/// * `results` - 解析結果のスライス
/// * `options` - 正規化オプション
///
/// # Returns
/// 正規化結果（修正内容と統計情報）
pub fn normalize_results(
    results: &[AnalysisResult],
    _options: &NormalizationOptions,
) -> NormalizationResult {
    let corrections = Vec::new();
    let stats = NormalizationStats {
        total_records: results.len(),
        ..Default::default()
    };

    // TODO: グループ単位での計測値統一を実装予定
    // - 温度管理: 3枚単位（全景+ボードアップ+温度計アップ）で同じ計測値を共有
    // - 出来形管理: 同一測点のセット全体で最も明瞭なOCR値を採用

    NormalizationResult { corrections, stats }
}

/// 修正を適用する
///
/// # Arguments
/// * `results` - 解析結果（変更される）
/// * `corrections` - 適用する修正リスト
pub fn apply_corrections(
    results: &mut [AnalysisResult],
    corrections: &[NormalizationCorrection],
) {
    for correction in corrections {
        if let Some(result) = results.iter_mut().find(|r| r.file_name == correction.file_name) {
            match correction.field {
                CorrectionField::Measurements => result.measurements = correction.corrected.clone(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_results_returns_empty() {
        let results = vec![
            AnalysisResult {
                file_name: "photo1.jpg".to_string(),
                measurements: "温度: 160℃".to_string(),
                ..Default::default()
            },
        ];

        let options = NormalizationOptions::default();
        let result = normalize_results(&results, &options);

        assert_eq!(result.stats.total_records, 1);
        assert!(result.corrections.is_empty());
    }
}
