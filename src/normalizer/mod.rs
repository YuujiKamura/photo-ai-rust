//! 後解析（正規化）モジュール
//!
//! 個別画像解析後に、測点・計測値・工種の整合性を統一する。
//!
//! ## 処理フロー
//! 1. 測点の最頻出値統一・OCR修正
//! 2. 計測値（温度・寸法）の保護
//! 3. 工種・種別の表記揺れ統一

pub mod station;
pub mod measurements;
pub mod work_type;

use crate::analyzer::AnalysisResult;
use std::collections::HashMap;

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
    Station,
    WorkType,
    Variety,
    Detail,
    Remarks,
}

impl std::fmt::Display for CorrectionField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CorrectionField::Station => write!(f, "測点"),
            CorrectionField::WorkType => write!(f, "工種"),
            CorrectionField::Variety => write!(f, "種別"),
            CorrectionField::Detail => write!(f, "細別"),
            CorrectionField::Remarks => write!(f, "備考"),
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
    /// 測点の修正数
    pub station_corrections: usize,
    /// 工種の修正数
    pub work_type_corrections: usize,
    /// スキップしたレコード数（計測値保護）
    pub skipped_due_to_measurements: usize,
}

/// 正規化オプション
#[derive(Debug, Clone)]
pub struct NormalizationOptions {
    /// 測点の正規化を有効にする
    pub normalize_station: bool,
    /// 工種・種別の統一を有効にする
    pub normalize_work_type: bool,
    /// 統一の閾値（この割合以上一致で統一）
    pub threshold: f64,
    /// 計測値を含むレコードの保護
    pub protect_measurements: bool,
}

impl Default for NormalizationOptions {
    fn default() -> Self {
        Self {
            normalize_station: true,
            normalize_work_type: true,
            threshold: 0.6, // 60%以上で統一
            protect_measurements: true,
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
    options: &NormalizationOptions,
) -> NormalizationResult {
    let mut corrections = Vec::new();
    let mut stats = NormalizationStats {
        total_records: results.len(),
        ..Default::default()
    };

    // 計測値を含むファイル名を収集（保護対象）
    let protected_files: std::collections::HashSet<&str> = if options.protect_measurements {
        results
            .iter()
            .filter(|r| measurements::contains_measurement(&r.remarks) ||
                       measurements::contains_measurement(&r.measurements))
            .map(|r| r.file_name.as_str())
            .collect()
    } else {
        std::collections::HashSet::new()
    };
    stats.skipped_due_to_measurements = protected_files.len();

    // 測点の正規化
    if options.normalize_station {
        let station_corrections = station::normalize_stations(results, options.threshold, &protected_files);
        stats.station_corrections = station_corrections.len();
        corrections.extend(station_corrections);
    }

    // 工種・種別の統一
    if options.normalize_work_type {
        let work_type_corrections = work_type::normalize_work_types(results, options.threshold, &protected_files);
        stats.work_type_corrections = work_type_corrections.len();
        corrections.extend(work_type_corrections);
    }

    // 修正されたレコード数を計算
    let corrected_files: std::collections::HashSet<&str> = corrections
        .iter()
        .map(|c| c.file_name.as_str())
        .collect();
    stats.corrected_records = corrected_files.len();

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
        // 対応するレコードを検索
        if let Some(result) = results.iter_mut().find(|r| r.file_name == correction.file_name) {
            match correction.field {
                CorrectionField::Station => result.station = correction.corrected.clone(),
                CorrectionField::WorkType => result.work_type = correction.corrected.clone(),
                CorrectionField::Variety => result.variety = correction.corrected.clone(),
                CorrectionField::Detail => result.detail = correction.corrected.clone(),
                CorrectionField::Remarks => result.remarks = correction.corrected.clone(),
            }
        }
    }
}

/// 最頻出値を取得する
pub fn find_most_frequent<'a>(values: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut total = 0;

    for value in values {
        if !value.is_empty() {
            *counts.entry(value).or_insert(0) += 1;
            total += 1;
        }
    }

    if total == 0 {
        return None;
    }

    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(value, _)| value.to_string())
}

/// 最頻出値とその割合を取得する
pub fn find_most_frequent_with_ratio<'a>(
    values: impl Iterator<Item = &'a str>,
) -> Option<(String, f64)> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut total = 0;

    for value in values {
        if !value.is_empty() {
            *counts.entry(value).or_insert(0) += 1;
            total += 1;
        }
    }

    if total == 0 {
        return None;
    }

    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(value, count)| (value.to_string(), count as f64 / total as f64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_most_frequent() {
        let values = vec!["A", "B", "A", "C", "A"];
        assert_eq!(find_most_frequent(values.iter().copied()), Some("A".to_string()));
    }

    #[test]
    fn test_find_most_frequent_empty() {
        let values: Vec<&str> = vec![];
        assert_eq!(find_most_frequent(values.iter().copied()), None);
    }

    #[test]
    fn test_find_most_frequent_with_ratio() {
        let values = vec!["A", "A", "B", "A", "C"];
        let result = find_most_frequent_with_ratio(values.iter().copied());
        assert!(result.is_some());
        let (value, ratio) = result.unwrap();
        assert_eq!(value, "A");
        assert!((ratio - 0.6).abs() < 0.01);
    }
}
