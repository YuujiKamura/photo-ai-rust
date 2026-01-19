//! 測点（Station）の正規化
//!
//! - 最頻出パターンを基準として統一
//! - OCRエラー修正（`0` vs `O`, `1` vs `l`）
//! - 表記揺れ統一（`No.X+XX` vs `No.X.XX`）

use super::{CorrectionField, NormalizationCorrection, find_most_frequent_with_ratio};
use crate::analyzer::AnalysisResult;
use regex::Regex;
use std::collections::HashSet;

/// 測点を正規化する
///
/// # Arguments
/// * `results` - 解析結果のスライス
/// * `threshold` - 統一の閾値
/// * `protected_files` - 保護対象のファイル名セット
pub fn normalize_stations(
    results: &[AnalysisResult],
    threshold: f64,
    protected_files: &HashSet<&str>,
) -> Vec<NormalizationCorrection> {
    let mut corrections = Vec::new();

    // 測点を正規化形式に変換してグループ化
    let normalized_stations: Vec<(&AnalysisResult, String)> = results
        .iter()
        .filter(|r| !r.station.is_empty())
        .map(|r| (r, normalize_station_format(&r.station)))
        .collect();

    if normalized_stations.is_empty() {
        return corrections;
    }

    // 正規化形式での最頻出値を取得
    let (most_frequent_normalized, ratio) = match find_most_frequent_with_ratio(
        normalized_stations.iter().map(|(_, s)| s.as_str())
    ) {
        Some(result) => result,
        None => return corrections,
    };

    // 閾値未満なら統一しない
    if ratio < threshold {
        return corrections;
    }

    // 最頻出の正規化形式に対応する元の表記を取得
    let target_station = normalized_stations
        .iter()
        .find(|(_, normalized)| *normalized == most_frequent_normalized)
        .map(|(r, _)| r.station.clone())
        .unwrap_or(most_frequent_normalized.clone());

    // 異なる測点を修正（元の値が異なる場合）
    for result in results {
        if result.station.is_empty() {
            continue;
        }

        // 保護対象はスキップ
        if protected_files.contains(result.file_name.as_str()) {
            continue;
        }

        // 元の値が異なる場合に修正（大文字小文字は無視）
        if result.station.to_lowercase() != target_station.to_lowercase() {
            corrections.push(NormalizationCorrection {
                file_name: result.file_name.clone(),
                field: CorrectionField::Station,
                original: result.station.clone(),
                corrected: target_station.clone(),
                reason: format!(
                    "最頻出測点「{}」に統一（元: {}）",
                    target_station, result.station
                ),
            });
        }
    }

    corrections
}

/// 測点を正規化形式に変換（比較用）
///
/// - 全角→半角変換
/// - 大文字→小文字
/// - 区切り文字統一（+, ., - → +）
/// - OCRエラー修正（O→0, l→1, I→1）
pub fn normalize_station_format(station: &str) -> String {
    let mut result = station.to_string();

    // 全角→半角
    result = result
        .chars()
        .map(|c| match c {
            '０'..='９' => ((c as u32) - '０' as u32 + '0' as u32) as u8 as char,
            'Ａ'..='Ｚ' => ((c as u32) - 'Ａ' as u32 + 'A' as u32) as u8 as char,
            'ａ'..='ｚ' => ((c as u32) - 'ａ' as u32 + 'a' as u32) as u8 as char,
            '＋' => '+',
            '．' => '.',
            '－' => '-',
            '　' => ' ',
            _ => c,
        })
        .collect();

    // 小文字化
    result = result.to_lowercase();

    // OCRエラー修正
    // 数字の文脈でO→0, l→1, I→1
    result = fix_ocr_errors(&result);

    // 区切り文字統一（. - → +）
    // No.X.XX → No.X+XX
    lazy_static::lazy_static! {
        static ref SEPARATOR_RE: Regex = Regex::new(r"no\.(\d+)[.\-](\d+)").unwrap();
    }
    result = SEPARATOR_RE.replace_all(&result, "no.$1+$2").to_string();

    result
}

/// OCRエラーを修正
fn fix_ocr_errors(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        let prev_is_digit = i > 0 && chars[i - 1].is_ascii_digit();
        let next_is_digit = i + 1 < chars.len() && chars[i + 1].is_ascii_digit();
        let in_number_context = prev_is_digit || next_is_digit;

        let fixed = match c {
            'o' | 'O' if in_number_context => '0',
            'l' | 'I' if in_number_context => '1',
            _ => c,
        };
        result.push(fixed);
    }

    result
}

/// 測点パターンを検出
pub fn detect_station_pattern(station: &str) -> Option<StationPattern> {
    lazy_static::lazy_static! {
        // No.X+XX 形式
        static ref PATTERN_PLUS: Regex = Regex::new(r"(?i)no\.?\s*(\d+)\+(\d+)").unwrap();
        // No.X.XX 形式
        static ref PATTERN_DOT: Regex = Regex::new(r"(?i)no\.?\s*(\d+)\.(\d+)").unwrap();
        // No.X-XX 形式
        static ref PATTERN_DASH: Regex = Regex::new(r"(?i)no\.?\s*(\d+)-(\d+)").unwrap();
        // No.X 形式（整数のみ）
        static ref PATTERN_INT: Regex = Regex::new(r"(?i)no\.?\s*(\d+)$").unwrap();
    }

    if let Some(caps) = PATTERN_PLUS.captures(station) {
        return Some(StationPattern::Plus(
            caps[1].parse().unwrap_or(0),
            caps[2].parse().unwrap_or(0),
        ));
    }
    if let Some(caps) = PATTERN_DOT.captures(station) {
        return Some(StationPattern::Dot(
            caps[1].parse().unwrap_or(0),
            caps[2].parse().unwrap_or(0),
        ));
    }
    if let Some(caps) = PATTERN_DASH.captures(station) {
        return Some(StationPattern::Dash(
            caps[1].parse().unwrap_or(0),
            caps[2].parse().unwrap_or(0),
        ));
    }
    if let Some(caps) = PATTERN_INT.captures(station) {
        return Some(StationPattern::Integer(caps[1].parse().unwrap_or(0)));
    }

    None
}

/// 測点のパターン
#[derive(Debug, Clone, PartialEq)]
pub enum StationPattern {
    /// No.X+XX
    Plus(u32, u32),
    /// No.X.XX
    Dot(u32, u32),
    /// No.X-XX
    Dash(u32, u32),
    /// No.X
    Integer(u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_station_format() {
        assert_eq!(normalize_station_format("No.10+50"), "no.10+50");
        assert_eq!(normalize_station_format("NO.10.50"), "no.10+50");
        assert_eq!(normalize_station_format("no.10-50"), "no.10+50");
        assert_eq!(normalize_station_format("Ｎｏ．１０＋５０"), "no.10+50");
    }

    #[test]
    fn test_fix_ocr_errors() {
        assert_eq!(fix_ocr_errors("no.1O+5O"), "no.10+50");
        assert_eq!(fix_ocr_errors("no.l0+50"), "no.10+50");
    }

    #[test]
    fn test_detect_station_pattern() {
        assert_eq!(
            detect_station_pattern("No.10+50"),
            Some(StationPattern::Plus(10, 50))
        );
        assert_eq!(
            detect_station_pattern("No.10.50"),
            Some(StationPattern::Dot(10, 50))
        );
        assert_eq!(
            detect_station_pattern("No.10"),
            Some(StationPattern::Integer(10))
        );
    }

    #[test]
    fn test_normalize_stations() {
        let results = vec![
            AnalysisResult {
                file_name: "photo1.jpg".to_string(),
                station: "No.10+50".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "photo2.jpg".to_string(),
                station: "No.10+50".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "photo3.jpg".to_string(),
                station: "No.10.50".to_string(), // 異なる表記
                ..Default::default()
            },
        ];

        let protected = HashSet::new();
        let corrections = normalize_stations(&results, 0.6, &protected);

        assert_eq!(corrections.len(), 1);
        assert_eq!(corrections[0].file_name, "photo3.jpg");
        assert_eq!(corrections[0].corrected, "No.10+50");
    }
}
