//! 工種・種別の正規化
//!
//! - 最頻出の工種・種別に統一
//! - 表記揺れの修正

use super::{CorrectionField, NormalizationCorrection, find_most_frequent_with_ratio};
use crate::analyzer::AnalysisResult;
use std::collections::HashSet;

/// 工種・種別を正規化する
///
/// # Arguments
/// * `results` - 解析結果のスライス
/// * `threshold` - 統一の閾値
/// * `protected_files` - 保護対象のファイル名セット
pub fn normalize_work_types(
    results: &[AnalysisResult],
    threshold: f64,
    protected_files: &HashSet<&str>,
) -> Vec<NormalizationCorrection> {
    let mut corrections = Vec::new();

    // 工種の正規化
    corrections.extend(normalize_field(
        results,
        threshold,
        protected_files,
        |r| &r.work_type,
        CorrectionField::WorkType,
    ));

    // 種別の正規化
    corrections.extend(normalize_field(
        results,
        threshold,
        protected_files,
        |r| &r.variety,
        CorrectionField::Variety,
    ));

    // 細別の正規化
    corrections.extend(normalize_field(
        results,
        threshold,
        protected_files,
        |r| &r.detail,
        CorrectionField::Detail,
    ));

    corrections
}

/// 特定フィールドを最頻出値に統一
fn normalize_field<F>(
    results: &[AnalysisResult],
    threshold: f64,
    protected_files: &HashSet<&str>,
    field_accessor: F,
    field_type: CorrectionField,
) -> Vec<NormalizationCorrection>
where
    F: Fn(&AnalysisResult) -> &String,
{
    let mut corrections = Vec::new();

    // 空でない値のみを対象
    let values: Vec<&str> = results
        .iter()
        .map(|r| field_accessor(r).as_str())
        .filter(|v| !v.is_empty())
        .collect();

    if values.is_empty() {
        return corrections;
    }

    // 最頻出値を取得
    let (most_frequent, ratio) = match find_most_frequent_with_ratio(values.iter().copied()) {
        Some(result) => result,
        None => return corrections,
    };

    // 閾値未満なら統一しない
    if ratio < threshold {
        return corrections;
    }

    // 異なる値を修正
    for result in results {
        let value = field_accessor(result);
        if value.is_empty() {
            continue;
        }

        // 保護対象はスキップ
        if protected_files.contains(result.file_name.as_str()) {
            continue;
        }

        if value != &most_frequent {
            corrections.push(NormalizationCorrection {
                file_name: result.file_name.clone(),
                field: field_type.clone(),
                original: value.clone(),
                corrected: most_frequent.clone(),
                reason: format!(
                    "最頻出の{}「{}」に統一（元: {}）",
                    field_type, most_frequent, value
                ),
            });
        }
    }

    corrections
}

/// 工種の表記揺れを正規化形式に変換
pub fn normalize_work_type_name(name: &str) -> String {
    let mut result = name.to_string();

    // 全角→半角スペース
    result = result.replace('　', " ");

    // 前後の空白を除去
    result = result.trim().to_string();

    // 連続スペースを単一に
    while result.contains("  ") {
        result = result.replace("  ", " ");
    }

    result
}

/// 類似度を計算（編集距離ベース）
pub fn similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let distance = levenshtein_distance(a, b);
    let max_len = a.chars().count().max(b.chars().count());

    1.0 - (distance as f64 / max_len as f64)
}

/// レーベンシュタイン距離を計算
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut matrix = vec![vec![0; b_len + 1]; a_len + 1];

    for i in 0..=a_len {
        matrix[i][0] = i;
    }
    for j in 0..=b_len {
        matrix[0][j] = j;
    }

    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[a_len][b_len]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_work_type_name() {
        assert_eq!(normalize_work_type_name("舗装工"), "舗装工");
        assert_eq!(normalize_work_type_name("　舗装工　"), "舗装工");
        assert_eq!(normalize_work_type_name("舗装  工"), "舗装 工");
    }

    #[test]
    fn test_similarity() {
        assert!((similarity("舗装工", "舗装工") - 1.0).abs() < 0.01);
        assert!(similarity("舗装工", "舗装補修工") > 0.5);
        assert!(similarity("舗装工", "区画線工") < 0.5);
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("", "abc"), 3);
        assert_eq!(levenshtein_distance("abc", "abc"), 0);
        assert_eq!(levenshtein_distance("abc", "abd"), 1);
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn test_normalize_work_types() {
        let results = vec![
            AnalysisResult {
                file_name: "photo1.jpg".to_string(),
                work_type: "舗装工".to_string(),
                variety: "舗装打換え工".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "photo2.jpg".to_string(),
                work_type: "舗装工".to_string(),
                variety: "舗装打換え工".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "photo3.jpg".to_string(),
                work_type: "舗装工".to_string(),
                variety: "舗装打替え工".to_string(), // 表記揺れ
                ..Default::default()
            },
        ];

        let protected = HashSet::new();
        let corrections = normalize_work_types(&results, 0.6, &protected);

        // 種別の修正が1件
        assert_eq!(corrections.len(), 1);
        assert_eq!(corrections[0].file_name, "photo3.jpg");
        assert_eq!(corrections[0].field, CorrectionField::Variety);
        assert_eq!(corrections[0].corrected, "舗装打換え工");
    }
}
