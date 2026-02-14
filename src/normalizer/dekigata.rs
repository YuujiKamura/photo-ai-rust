//! 出来形管理用紙のOCRテキストから測定値を解析・フォーマットする
//!
//! ## 対応する管理用紙
//! - 路面切削工: V1～V5の切削高 + 左右幅員
//!
//! ## 処理フロー
//! 1. detected_text(カンマ区切りOCR)をパース → DekigataValues
//! 2. 車線判定（左: V1～V3+W1 / 右: V4～V5+W2）
//! 3. 4行フォーマットのmeasurements文字列を生成
//! 4. 同一測点3枚セットに統一適用

use regex::Regex;

/// 車線（左/右）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    /// 左車線: V1～V3, W1（左幅員）
    Left,
    /// 右車線: V4～V5, W2（右幅員）
    Right,
}

impl std::fmt::Display for Lane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Lane::Left => write!(f, "左車線"),
            Lane::Right => write!(f, "右車線"),
        }
    }
}

/// 出来形管理用紙から抽出した値
#[derive(Debug, Clone)]
pub struct DekigataValues {
    /// 測点名（"No.1" 等）
    pub station: String,
    /// 切削高(設計) V1～V5
    pub v_design: [Option<f64>; 5],
    /// 切削高(実施) V1～V5
    pub v_actual: [Option<f64>; 5],
    /// 左幅員 設計
    pub width_left_design: Option<f64>,
    /// 左幅員 実測
    pub width_left_actual: Option<f64>,
    /// 右幅員 設計
    pub width_right_design: Option<f64>,
    /// 右幅員 実測
    pub width_right_actual: Option<f64>,
}

impl Default for DekigataValues {
    fn default() -> Self {
        Self {
            station: String::new(),
            v_design: [None; 5],
            v_actual: [None; 5],
            width_left_design: None,
            width_left_actual: None,
            width_right_design: None,
            width_right_actual: None,
        }
    }
}

/// スペース区切りテキストをキーワードでセクション分割する
///
/// 切削高(設計)、切削高(実施)、左幅員、右幅員をセクション区切りとして扱う
fn split_by_keywords(text: &str) -> Vec<String> {
    let keywords = [
        "切削高(設計)", "切削高（設計）",
        "切削高(実施)", "切削高（実施）",
        "左幅員", "右幅員",
    ];

    // キーワードの出現位置を収集
    let mut positions: Vec<usize> = Vec::new();
    for kw in &keywords {
        let mut start = 0;
        while let Some(pos) = text[start..].find(kw) {
            positions.push(start + pos);
            start += pos + kw.len();
        }
    }
    positions.sort();
    positions.dedup();

    if positions.is_empty() {
        return vec![text.to_string()];
    }

    let mut sections = Vec::new();
    // テキスト先頭～最初のキーワードまで（測点情報等）
    if positions[0] > 0 {
        sections.push(text[..positions[0]].trim().to_string());
    }
    // 各キーワード区間
    for i in 0..positions.len() {
        let start = positions[i];
        let end = if i + 1 < positions.len() {
            positions[i + 1]
        } else {
            text.len()
        };
        sections.push(text[start..end].trim().to_string());
    }
    sections
}

/// detected_textから出来形管理用紙の値をパースする
///
/// # 入力フォーマット例
/// ```text
/// 出来形管理用紙 No.1, GH=9.916 FH=9.911,
/// 切削高(設計) V1=9.819 V2=9.842 V3=9.861 V4=9.826 V5=9.785,
/// 切削高(実施) V1=9.815 V2=9.842 V3=9.860 V4=9.825 V5=9.780,
/// 左幅員 設計4.20 実測4.20, 右幅員 設計4.13 実測4.13
/// ```
pub fn parse_dekigata_ocr(text: &str) -> Option<DekigataValues> {
    if !text.contains("出来形管理用紙") {
        return None;
    }

    let mut values = DekigataValues::default();

    // 測点: "No.X" を抽出
    lazy_static::lazy_static! {
        static ref STATION_RE: Regex = Regex::new(
            r"出来形管理用紙\s*(No\.\d+)"
        ).unwrap();
        static ref V_RE: Regex = Regex::new(
            r"V(\d)=(\d+\.?\d*)"
        ).unwrap();
        static ref WIDTH_LEFT_RE: Regex = Regex::new(
            r"左幅員\s*設計\s*(\d+\.?\d*)\s*実測\s*(\d+\.?\d*)"
        ).unwrap();
        static ref WIDTH_RIGHT_RE: Regex = Regex::new(
            r"右幅員\s*設計\s*(\d+\.?\d*)\s*実測\s*(\d+\.?\d*)"
        ).unwrap();
        // 実測のみパターン（設計なし）
        static ref WIDTH_LEFT_ACTUAL_ONLY_RE: Regex = Regex::new(
            r"左幅員\s*実測\s*(\d+\.?\d*)"
        ).unwrap();
        static ref WIDTH_RIGHT_ACTUAL_ONLY_RE: Regex = Regex::new(
            r"右幅員\s*実測\s*(\d+\.?\d*)"
        ).unwrap();
        // GH=, FH= の値を除外するためのパターン
        static ref GH_FH_RE: Regex = Regex::new(
            r"[GF]H=\d+\.?\d*"
        ).unwrap();
        // 小数点付き数値
        static ref NUM_RE: Regex = Regex::new(
            r"(\d+\.\d+)"
        ).unwrap();
    }

    // 測点
    if let Some(caps) = STATION_RE.captures(text) {
        values.station = caps[1].to_string();
    }

    // セクション分割: カンマ or 切削高キーワードで分割
    // まずカンマで分割し、カンマがなければ全体を1セクションとして扱う
    let sections: Vec<String> = if text.contains(',') {
        text.split(',').map(|s| s.trim().to_string()).collect()
    } else {
        // スペース区切りの場合: 切削高キーワードでセクションに分割
        split_by_keywords(text)
    };

    for section in &sections {
        let is_design = section.contains("切削高(設計)") || section.contains("切削高（設計）");
        let is_actual = section.contains("切削高(実施)") || section.contains("切削高（実施）");

        if is_design || is_actual {
            let target = if is_design {
                &mut values.v_design
            } else {
                &mut values.v_actual
            };

            // V=ラベルありでマッチ
            let mut v_count = 0;
            for v_cap in V_RE.captures_iter(section) {
                let idx: usize = v_cap[1].parse().unwrap_or(0);
                if idx >= 1 && idx <= 5 {
                    target[idx - 1] = v_cap[2].parse().ok();
                    v_count += 1;
                }
            }

            // V=ラベルなしフォールバック: GH=/FH=を除外した数値列を順番に割り当て
            if v_count == 0 {
                let clean = GH_FH_RE.replace_all(section, "").to_string();
                for (i, cap) in NUM_RE.captures_iter(&clean).enumerate() {
                    if i < 5 {
                        target[i] = cap[1].parse().ok();
                    }
                }
            }
        }

        // 幅員: 設計+実測パターン優先
        if let Some(caps) = WIDTH_LEFT_RE.captures(section) {
            values.width_left_design = caps[1].parse().ok();
            values.width_left_actual = caps[2].parse().ok();
        } else if let Some(caps) = WIDTH_LEFT_ACTUAL_ONLY_RE.captures(section) {
            values.width_left_actual = caps[1].parse().ok();
        }
        if let Some(caps) = WIDTH_RIGHT_RE.captures(section) {
            values.width_right_design = caps[1].parse().ok();
            values.width_right_actual = caps[2].parse().ok();
        } else if let Some(caps) = WIDTH_RIGHT_ACTUAL_ONLY_RE.captures(section) {
            values.width_right_actual = caps[1].parse().ok();
        }
    }

    // 最低限のデータがあるか確認（設計のみ or 実施のみでもOK）
    let has_design = values.v_design.iter().any(|v| v.is_some());
    let has_actual = values.v_actual.iter().any(|v| v.is_some());
    if has_design || has_actual {
        Some(values)
    } else {
        None
    }
}

/// 車線を自動判定する
///
/// ルール:
/// - 左幅員の実測があり右幅員の実測がない → 左車線
/// - 右幅員の実測があり左幅員の実測がない → 右車線
/// - 両方ある場合 → V値の差分で判定（実施値が設計値と異なるV点がある側）
/// - 判定不能 → None
pub fn determine_lane(values: &DekigataValues) -> Option<Lane> {
    let has_left = values.width_left_actual.is_some();
    let has_right = values.width_right_actual.is_some();

    match (has_left, has_right) {
        (true, false) => return Some(Lane::Left),
        (false, true) => return Some(Lane::Right),
        (false, false) => return None,
        _ => {} // 両方ある → V値で判定
    }

    // V1-V3の差分合計 vs V4-V5の差分合計で判定
    let left_diff: f64 = (0..3)
        .filter_map(|i| {
            let d = values.v_design[i]?;
            let a = values.v_actual[i]?;
            Some((d - a).abs())
        })
        .sum();
    let right_diff: f64 = (3..5)
        .filter_map(|i| {
            let d = values.v_design[i]?;
            let a = values.v_actual[i]?;
            Some((d - a).abs())
        })
        .sum();

    if left_diff > right_diff + 0.001 {
        Some(Lane::Left)
    } else if right_diff > left_diff + 0.001 {
        Some(Lane::Right)
    } else {
        None // 差が同程度で判定不能
    }
}

/// measurements文字列を生成する
///
/// # フォーマット（左車線の場合）
/// ```text
/// 左車線　切削基準高V1～V3 幅員W1
/// 設計: V1=9.819 V2=9.842 V3=9.861
/// 実施: V1=9.815 V2=9.842 V3=9.860
/// 幅員W1 設計: 4.20 実測: 4.20
/// ```
pub fn format_measurements(values: &DekigataValues, lane: Lane) -> String {
    match lane {
        Lane::Left => format_left(values),
        Lane::Right => format_right(values),
    }
}

fn format_v_values(values: &[Option<f64>], start_v_num: usize) -> String {
    values
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.map(|x| format!("V{}={:.3}", start_v_num + i, x)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_width(label: &str, design: Option<f64>, actual: Option<f64>) -> String {
    match (design, actual) {
        (Some(d), Some(a)) => format!("{} 設計: {:.2} 実測: {:.2}", label, d, a),
        (Some(d), None) => format!("{} 設計: {:.2}", label, d),
        _ => String::new(),
    }
}

fn format_left(values: &DekigataValues) -> String {
    let v_design = format_v_values(&values.v_design[0..3], 1);
    let v_actual = format_v_values(&values.v_actual[0..3], 1);
    let width = format_width(
        "幅員W1",
        values.width_left_design,
        values.width_left_actual,
    );

    format!(
        "左車線\u{3000}切削基準高 幅員W1\n設計: {}\n実施: {}\n{}",
        v_design, v_actual, width
    )
}

fn format_right(values: &DekigataValues) -> String {
    let v_design = format_v_values(&values.v_design[3..5], 4);
    let v_actual = format_v_values(&values.v_actual[3..5], 4);
    let width = format_width(
        "幅員W2",
        values.width_right_design,
        values.width_right_actual,
    );

    format!(
        "右車線\u{3000}切削基準高 幅員W2\n設計: {}\n実施: {}\n{}",
        v_design, v_actual, width
    )
}

/// 出来形管理写真かどうか判定
pub fn is_dekigata_photo(result: &crate::analyzer::AnalysisResult) -> bool {
    result.photo_category == "出来形管理写真"
        || result.remarks.contains("出来形")
        || result.detected_text.contains("出来形管理用紙")
}

/// 出来形管理用紙のアップ写真（detectedTextに詳細値がある写真）を特定
pub fn find_board_detail_photo(
    results: &[crate::analyzer::AnalysisResult],
    group_indices: &[usize],
) -> Option<usize> {
    // focusTarget が "出来形管理図表" or "出来形管理" で、detectedTextにV値がある写真
    group_indices.iter().copied().find(|&i| {
        let r = &results[i];
        r.detected_text.contains("切削高(設計)")
            || r.detected_text.contains("切削高（設計）")
            || r.detected_text.contains("切削高(実施)")
            || r.detected_text.contains("切削高（実施）")
    })
}

/// 同一測点の出来形管理写真セットのmeasurementsを統一する
///
/// # Returns
/// 生成したmeasurements文字列（統一に使用した値）。Noneならパース失敗
pub fn unify_dekigata_set(
    results: &[crate::analyzer::AnalysisResult],
    group_indices: &[usize],
    lane_override: Option<Lane>,
) -> Option<String> {
    // 管理用紙アップ写真を探す
    let detail_idx = find_board_detail_photo(results, group_indices)?;
    let detail = &results[detail_idx];

    // OCRテキストをパース
    let values = parse_dekigata_ocr(&detail.detected_text)?;

    // 車線判定
    let lane = lane_override.or_else(|| determine_lane(&values))?;

    // measurements生成
    Some(format_measurements(&values, lane))
}

#[cfg(test)]
mod tests {
    use super::*;

    // 0209 No.1（左車線）の実データ
    const OCR_0209_NO1: &str = "出来形管理用紙 No.1, GH=9.916 FH=9.911, 計画高(設計) V1=9.869 V2=9.892 V3=9.911 V4=9.876 V5=9.835, 切削高(設計) V1=9.819 V2=9.842 V3=9.861 V4=9.826 V5=9.785, 切削高(実施) V1=9.815 V2=9.842 V3=9.860 V4=9.825 V5=9.780, 左幅員 設計4.20 実測4.20, 右幅員 設計4.13 実測4.13";

    // 0211 No.5（右車線）の実データ
    const OCR_0211_NO5: &str = "出来形管理用紙 No.5, GH=10.088 FH=10.097, 計画高(設計) V1=10.019 V2=10.049 V3=10.097 V4=10.051 V5=10.024, 切削高(設計) V1=9.969 V2=9.999 V3=10.047 V4=10.001 V5=9.974, 切削高(実施) V1=9.965 V2=9.997 V3=10.045 V4=10.000 V5=9.970, 左幅員 設計3.20 実測3.20, 右幅員 設計3.18 実測3.18";

    // 0211 No.3（右車線）の実データ
    const OCR_0211_NO3: &str = "出来形管理用紙 No.3, GH=10.026 FH=10.055, 切削高(設計) V1=9.988 V2=9.995 V3=10.005 V4=9.955 V5=9.906, 切削高(実施) V1=9.985 V2=9.990 V3=10.002 V4=9.996 V5=9.902, 左幅員 設計3.20 実測3.20, 右幅員 設計3.90 実測3.90";

    #[test]
    fn test_parse_dekigata_ocr_left_lane() {
        let values = parse_dekigata_ocr(OCR_0209_NO1).unwrap();
        assert_eq!(values.station, "No.1");
        assert_eq!(values.v_design[0], Some(9.819));
        assert_eq!(values.v_design[4], Some(9.785));
        assert_eq!(values.v_actual[0], Some(9.815));
        assert_eq!(values.v_actual[4], Some(9.780));
        assert_eq!(values.width_left_design, Some(4.20));
        assert_eq!(values.width_left_actual, Some(4.20));
        assert_eq!(values.width_right_design, Some(4.13));
        assert_eq!(values.width_right_actual, Some(4.13));
    }

    #[test]
    fn test_parse_dekigata_ocr_right_lane() {
        let values = parse_dekigata_ocr(OCR_0211_NO5).unwrap();
        assert_eq!(values.station, "No.5");
        assert_eq!(values.v_design[3], Some(10.001));
        assert_eq!(values.v_design[4], Some(9.974));
        assert_eq!(values.v_actual[3], Some(10.000));
        assert_eq!(values.v_actual[4], Some(9.970));
        assert_eq!(values.width_right_design, Some(3.18));
        assert_eq!(values.width_right_actual, Some(3.18));
    }

    #[test]
    fn test_parse_non_dekigata() {
        assert!(parse_dekigata_ocr("温度 160.4℃").is_none());
        assert!(parse_dekigata_ocr("").is_none());
    }

    #[test]
    fn test_format_measurements_left() {
        let values = parse_dekigata_ocr(OCR_0209_NO1).unwrap();
        let m = format_measurements(&values, Lane::Left);
        assert!(m.starts_with("左車線"));
        assert!(m.starts_with("左車線\u{3000}切削基準高 幅員W1"));
        assert!(m.contains("設計: V1=9.819 V2=9.842 V3=9.861"));
        assert!(m.contains("実施: V1=9.815 V2=9.842 V3=9.860"));
        assert!(m.contains("幅員W1 設計: 4.20 実測: 4.20"));
    }

    #[test]
    fn test_format_measurements_right() {
        let values = parse_dekigata_ocr(OCR_0211_NO5).unwrap();
        let m = format_measurements(&values, Lane::Right);
        assert!(m.starts_with("右車線"));
        assert!(m.starts_with("右車線\u{3000}切削基準高 幅員W2"));
        assert!(m.contains("設計: V4=10.001 V5=9.974"));
        assert!(m.contains("実施: V4=10.000 V5=9.970"));
        assert!(m.contains("幅員W2 設計: 3.18 実測: 3.18"));
    }

    #[test]
    fn test_format_measurements_right_no3() {
        let values = parse_dekigata_ocr(OCR_0211_NO3).unwrap();
        let m = format_measurements(&values, Lane::Right);
        assert!(m.contains("設計: V4=9.955 V5=9.906"));
        assert!(m.contains("実施: V4=9.996 V5=9.902"));
        assert!(m.contains("幅員W2 設計: 3.90 実測: 3.90"));
    }

    #[test]
    fn test_format_exact_output_left() {
        let values = parse_dekigata_ocr(OCR_0209_NO1).unwrap();
        let m = format_measurements(&values, Lane::Left);
        let expected = "左車線\u{3000}切削基準高 幅員W1\n\
                        設計: V1=9.819 V2=9.842 V3=9.861\n\
                        実施: V1=9.815 V2=9.842 V3=9.860\n\
                        幅員W1 設計: 4.20 実測: 4.20";
        assert_eq!(m, expected);
    }

    // 0212 No.9: V=ラベルなし、幅員実測のみ
    const OCR_0212_NO9: &str = "出来形管理用紙 No.9 GH=10.414 FH=10.426 切削高(実施) 10.375 10.328 10.292 右幅員 実測 3.20";

    // 0212 No.11: V=ラベルなし
    const OCR_0212_NO11: &str = "出来形管理用紙 No.11 GH=10.669 FH=10.674 切削高(実施) 10.621 10.585 10.568 右幅員 実測 3.20";

    #[test]
    fn test_parse_dekigata_no_v_labels() {
        let values = parse_dekigata_ocr(OCR_0212_NO9).unwrap();
        assert_eq!(values.station, "No.9");
        // V=ラベルなしの数値がV1～V3に順番に入る
        assert_eq!(values.v_actual[0], Some(10.375));
        assert_eq!(values.v_actual[1], Some(10.328));
        assert_eq!(values.v_actual[2], Some(10.292));
        assert_eq!(values.width_right_actual, Some(3.20));
    }

    #[test]
    fn test_parse_dekigata_no11_no_v_labels() {
        let values = parse_dekigata_ocr(OCR_0212_NO11).unwrap();
        assert_eq!(values.station, "No.11");
        assert_eq!(values.v_actual[0], Some(10.621));
        assert_eq!(values.v_actual[1], Some(10.585));
        assert_eq!(values.v_actual[2], Some(10.568));
    }

    #[test]
    fn test_format_exact_output_right() {
        let values = parse_dekigata_ocr(OCR_0211_NO5).unwrap();
        let m = format_measurements(&values, Lane::Right);
        let expected = "右車線\u{3000}切削基準高 幅員W2\n\
                        設計: V4=10.001 V5=9.974\n\
                        実施: V4=10.000 V5=9.970\n\
                        幅員W2 設計: 3.18 実測: 3.18";
        assert_eq!(m, expected);
    }
}
