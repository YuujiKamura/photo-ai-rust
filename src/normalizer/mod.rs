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
    options: &NormalizationOptions,
) -> NormalizationResult {
    let mut corrections = Vec::new();
    let mut stats = NormalizationStats {
        total_records: results.len(),
        ..Default::default()
    };

    if options.unify_measurements {
        // 温度値のバリデーションと修正
        for result in results {
            // 温度写真かどうか判定
            let combined_text = format!("{} {} {}", result.remarks, result.description, result.detected_text);
            if measurements::is_temperature_photo(&combined_text) {
                // 温度種別を判定
                let temp_type = measurements::TemperatureType::from_text(&combined_text);

                // measurements フィールドの温度値を検証
                if !result.measurements.is_empty() {
                    if let Some(corrected) = measurements::validate_temperature(&result.measurements, temp_type.clone()) {
                        corrections.push(NormalizationCorrection {
                            file_name: result.file_name.clone(),
                            field: CorrectionField::Measurements,
                            original: result.measurements.clone(),
                            corrected: corrected.clone(),
                            reason: format!("温度値修正 ({:?}の妥当範囲外)", temp_type),
                        });
                        stats.measurement_corrections += 1;
                        stats.corrected_records += 1;
                    }
                }
            }
        }
    }

    if options.unify_measurements {
        // グループ単位での計測値統一
        // 温度管理: 3枚セット（全景+黒板アップ+温度計アップ）で黒板アップの値に統一
        let group_corrections = unify_measurements_by_group(results);
        for correction in group_corrections {
            if !corrections.iter().any(|c| c.file_name == correction.file_name) {
                stats.measurement_corrections += 1;
                stats.corrected_records += 1;
                corrections.push(correction);
            }
        }
    }

    NormalizationResult { corrections, stats }
}

/// 3枚セット内で黒板アップの計測値に統一する
///
/// 連続する同一remarks（温度種別）の写真をグループ化し、
/// focusTarget="黒板アップ"の値を他の写真に適用する
fn unify_measurements_by_group(results: &[AnalysisResult]) -> Vec<NormalizationCorrection> {
    let mut corrections = Vec::new();

    // 連続する同一remarksでグループ化
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current_group: Vec<usize> = Vec::new();
    let mut current_remarks: Option<&str> = None;

    for (i, result) in results.iter().enumerate() {
        // 温度写真判定: remarksまたはphoto_categoryで判定
        let is_temp = measurements::is_temperature_photo(&result.remarks)
            || (result.photo_category == "品質管理写真"
                && measurements::is_temperature_photo(&result.detected_text));

        if !is_temp {
            if !current_group.is_empty() {
                groups.push(std::mem::take(&mut current_group));
                current_remarks = None;
            }
            continue;
        }

        let remarks = result.remarks.as_str();
        if current_remarks == Some(remarks) {
            current_group.push(i);
        } else {
            if !current_group.is_empty() {
                groups.push(std::mem::take(&mut current_group));
            }
            current_group.push(i);
            current_remarks = Some(remarks);
        }
    }
    if !current_group.is_empty() {
        groups.push(current_group);
    }

    // 各グループで黒板アップの値に統一
    for group in groups {
        // 黒板アップを探す
        let board_up = group.iter().find(|&&i| {
            results[i].focus_target == "黒板アップ"
        });

        if let Some(&board_idx) = board_up {
            let source_value = &results[board_idx].measurements;
            if source_value.is_empty() {
                continue;
            }

            // 他の写真の値を統一
            for &idx in &group {
                if idx == board_idx {
                    continue;
                }
                let target = &results[idx];
                if target.measurements != *source_value && !target.measurements.is_empty() {
                    corrections.push(NormalizationCorrection {
                        file_name: target.file_name.clone(),
                        field: CorrectionField::Measurements,
                        original: target.measurements.clone(),
                        corrected: source_value.clone(),
                        reason: format!(
                            "黒板アップ({})の値に統一",
                            results[board_idx].file_name
                        ),
                    });
                }
            }
        }
    }

    corrections
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

    #[test]
    fn test_unify_measurements_by_group() {
        // 3枚セット: 全景(147.6℃)、黒板アップ(149.6℃)、温度計アップ(149.6℃)
        let results = vec![
            AnalysisResult {
                file_name: "RIMG0188.JPG".to_string(),
                remarks: "初期締固め前温度".to_string(),
                measurements: "147.6℃".to_string(),
                focus_target: "全景".to_string(),
                photo_category: "品質管理写真".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "RIMG0189.JPG".to_string(),
                remarks: "初期締固め前温度".to_string(),
                measurements: "149.6℃".to_string(),
                focus_target: "黒板アップ".to_string(),
                photo_category: "品質管理写真".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "RIMG0190.JPG".to_string(),
                remarks: "初期締固め前温度".to_string(),
                measurements: "149.6℃".to_string(),
                focus_target: "温度計アップ".to_string(),
                photo_category: "品質管理写真".to_string(),
                ..Default::default()
            },
        ];

        let corrections = unify_measurements_by_group(&results);

        // 全景の値が黒板アップの値に修正されるべき
        assert_eq!(corrections.len(), 1);
        assert_eq!(corrections[0].file_name, "RIMG0188.JPG");
        assert_eq!(corrections[0].original, "147.6℃");
        assert_eq!(corrections[0].corrected, "149.6℃");
    }

    #[test]
    fn test_unify_measurements_multiple_groups() {
        // 2つの温度グループ
        let results = vec![
            // 到着温度グループ
            AnalysisResult {
                file_name: "IMG001.JPG".to_string(),
                remarks: "到着温度".to_string(),
                measurements: "160.0℃".to_string(),
                focus_target: "全景".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "IMG002.JPG".to_string(),
                remarks: "到着温度".to_string(),
                measurements: "160.7℃".to_string(),
                focus_target: "黒板アップ".to_string(),
                ..Default::default()
            },
            // 敷均し温度グループ
            AnalysisResult {
                file_name: "IMG003.JPG".to_string(),
                remarks: "敷均し温度".to_string(),
                measurements: "155.0℃".to_string(),
                focus_target: "全景".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "IMG004.JPG".to_string(),
                remarks: "敷均し温度".to_string(),
                measurements: "155.4℃".to_string(),
                focus_target: "黒板アップ".to_string(),
                ..Default::default()
            },
        ];

        let corrections = unify_measurements_by_group(&results);

        // 各グループで1件ずつ修正
        assert_eq!(corrections.len(), 2);
        assert_eq!(corrections[0].file_name, "IMG001.JPG");
        assert_eq!(corrections[0].corrected, "160.7℃");
        assert_eq!(corrections[1].file_name, "IMG003.JPG");
        assert_eq!(corrections[1].corrected, "155.4℃");
    }
}
