//! 解析練度評価モジュール
//!
//! パイプライン出力とGT（Ground Truth）を比較し、フィールド別の精度を算出する。
//! file_name をキーにして写真を突合し、指定フィールドの一致率を返す。

use crate::types::AnalysisResult;
use std::collections::HashMap;

/// 評価対象フィールド
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvalField {
    Remarks,
    Station,
    Measurements,
    WorkType,
    Variety,
    Subphase,
    PhotoCategory,
    FocusTarget,
}

impl EvalField {
    pub fn name(&self) -> &str {
        match self {
            Self::Remarks => "remarks",
            Self::Station => "station",
            Self::Measurements => "measurements",
            Self::WorkType => "workType",
            Self::Variety => "variety",
            Self::Subphase => "subphase",
            Self::PhotoCategory => "photoCategory",
            Self::FocusTarget => "focusTarget",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "remarks" => Some(Self::Remarks),
            "station" => Some(Self::Station),
            "measurements" => Some(Self::Measurements),
            "workType" | "work_type" => Some(Self::WorkType),
            "variety" => Some(Self::Variety),
            "subphase" => Some(Self::Subphase),
            "photoCategory" | "photo_category" => Some(Self::PhotoCategory),
            "focusTarget" | "focus_target" => Some(Self::FocusTarget),
            _ => None,
        }
    }

    /// Issue #98 のデフォルト評価フィールド: remarks, station, measurements
    pub fn default_fields() -> Vec<Self> {
        vec![Self::Remarks, Self::Station, Self::Measurements]
    }

    /// 全フィールド
    pub fn all() -> Vec<Self> {
        vec![
            Self::WorkType,
            Self::Variety,
            Self::Subphase,
            Self::Remarks,
            Self::Station,
            Self::Measurements,
            Self::PhotoCategory,
            Self::FocusTarget,
        ]
    }

    fn get_value<'a>(&self, r: &'a AnalysisResult) -> &'a str {
        match self {
            Self::Remarks => &r.remarks,
            Self::Station => &r.station,
            Self::Measurements => &r.measurements,
            Self::WorkType => &r.work_type,
            Self::Variety => &r.variety,
            Self::Subphase => &r.subphase,
            Self::PhotoCategory => &r.photo_category,
            Self::FocusTarget => &r.focus_target,
        }
    }
}

impl std::fmt::Display for EvalField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// 1フィールドの評価結果
#[derive(Debug)]
pub struct FieldResult {
    pub field: EvalField,
    pub total: usize,
    pub matched: usize,
}

impl FieldResult {
    pub fn accuracy(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.matched as f64 / self.total as f64 * 100.0
        }
    }
}

/// 1枚の写真×1フィールドの不一致
#[derive(Debug)]
pub struct Diff {
    pub file_name: String,
    pub field: EvalField,
    pub got: String,
    pub expected: String,
}

/// 評価レポート
#[derive(Debug)]
pub struct EvaluationReport {
    pub total_photos: usize,
    pub matched_photos: usize,
    pub missing_photos: usize,
    pub field_results: Vec<FieldResult>,
    pub diffs: Vec<Diff>,
}

/// 測点の正規化（ゆらぎ吸収）
///
/// - "No.1 右車線" → "No.1R"
/// - "No.1 L" → "No.1L"
/// - 全角→半角, スペース除去
fn normalize_station(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        return String::new();
    }

    // 全角→半角 (数字, アルファベット, ピリオド)
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '０'..='９' => result.push((ch as u32 - '０' as u32 + '0' as u32) as u8 as char),
            'Ａ'..='Ｚ' => result.push((ch as u32 - 'Ａ' as u32 + 'A' as u32) as u8 as char),
            'ａ'..='ｚ' => result.push((ch as u32 - 'ａ' as u32 + 'a' as u32) as u8 as char),
            '．' => result.push('.'),
            _ => result.push(ch),
        }
    }

    // 車線表記の正規化
    let result = result
        .replace("右車線", "R")
        .replace("左車線", "L")
        .replace(" R", "R")
        .replace(" L", "L")
        .replace(" 右", "R")
        .replace(" 左", "L");

    // スペース除去
    result.replace(' ', "")
}

/// 2つの値を比較（フィールドに応じた正規化を適用）
fn compare_field(pipe_val: &str, gt_val: &str, field: EvalField) -> bool {
    let pv = pipe_val.trim();
    let gv = gt_val.trim();

    // 両方空なら一致
    if pv.is_empty() && gv.is_empty() {
        return true;
    }

    match field {
        EvalField::Station => normalize_station(pv) == normalize_station(gv),
        _ => pv == gv,
    }
}

/// パイプライン出力とGTを比較して精度評価レポートを生成
///
/// - `pipeline`: パイプライン出力のAnalysisResult配列
/// - `gt`: Ground TruthのAnalysisResult配列
/// - `fields`: 評価対象フィールド（空なら全フィールド）
pub fn evaluate(
    pipeline: &[AnalysisResult],
    gt: &[AnalysisResult],
    fields: &[EvalField],
) -> EvaluationReport {
    let fields = if fields.is_empty() {
        EvalField::default_fields()
    } else {
        fields.to_vec()
    };

    // パイプラインをfile_nameでインデックス化
    let pipe_map: HashMap<&str, &AnalysisResult> = pipeline
        .iter()
        .filter(|r| !r.file_name.is_empty())
        .map(|r| (r.file_name.as_str(), r))
        .collect();

    // GTのうちfile_nameが空でないものだけ対象
    let gt_items: Vec<&AnalysisResult> = gt
        .iter()
        .filter(|r| !r.file_name.is_empty())
        .collect();

    let total_photos = gt_items.len();
    let mut matched_photos = 0;
    let mut missing_photos = 0;
    let mut field_matched: HashMap<EvalField, usize> = HashMap::new();
    let mut field_total: HashMap<EvalField, usize> = HashMap::new();
    let mut diffs = Vec::new();

    for &field in &fields {
        field_matched.insert(field, 0);
        field_total.insert(field, 0);
    }

    for gt_item in &gt_items {
        match pipe_map.get(gt_item.file_name.as_str()) {
            None => {
                missing_photos += 1;
            }
            Some(pipe_item) => {
                let mut all_match = true;
                for &field in &fields {
                    let gt_val = field.get_value(gt_item);
                    let pipe_val = field.get_value(pipe_item);
                    *field_total.get_mut(&field).unwrap() += 1;

                    if compare_field(pipe_val, gt_val, field) {
                        *field_matched.get_mut(&field).unwrap() += 1;
                    } else {
                        all_match = false;
                        diffs.push(Diff {
                            file_name: gt_item.file_name.clone(),
                            field,
                            got: pipe_val.to_string(),
                            expected: gt_val.to_string(),
                        });
                    }
                }
                if all_match {
                    matched_photos += 1;
                }
            }
        }
    }

    let field_results: Vec<FieldResult> = fields
        .iter()
        .map(|&field| FieldResult {
            field,
            total: field_total[&field],
            matched: field_matched[&field],
        })
        .collect();

    EvaluationReport {
        total_photos,
        matched_photos,
        missing_photos,
        field_results,
        diffs,
    }
}

/// レポートを表示
pub fn print_report(report: &EvaluationReport) {
    println!("=== 解析練度評価 ===");
    println!("対象: {}枚", report.total_photos);
    if report.missing_photos > 0 {
        println!("未照合: {}枚", report.missing_photos);
    }
    println!();

    for fr in &report.field_results {
        println!(
            "  {:<15} {:>3}/{:<3} 一致 ({:.1}%)",
            format!("{}:", fr.field.name()),
            fr.matched,
            fr.total,
            fr.accuracy()
        );
    }

    if !report.diffs.is_empty() {
        println!("\n不一致詳細:");
        for d in &report.diffs {
            let got = if d.got.is_empty() { "(空)" } else { &d.got };
            let expected = if d.expected.is_empty() { "(空)" } else { &d.expected };
            let got_trunc: String = got.chars().take(50).collect();
            let exp_trunc: String = expected.chars().take(50).collect();
            println!(
                "  {} {}: \"{}\" → GT: \"{}\"",
                d.file_name, d.field.name(), got_trunc, exp_trunc
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AnalysisResult;

    fn make_result(file_name: &str, remarks: &str, station: &str, measurements: &str) -> AnalysisResult {
        AnalysisResult {
            file_name: file_name.to_string(),
            remarks: remarks.to_string(),
            station: station.to_string(),
            measurements: measurements.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_normalize_station() {
        assert_eq!(normalize_station("No.1 右車線"), "No.1R");
        assert_eq!(normalize_station("No.1 L"), "No.1L");
        assert_eq!(normalize_station("No.1R"), "No.1R");
        assert_eq!(normalize_station(""), "");
        assert_eq!(normalize_station("取付道路No.1"), "取付道路No.1");
    }

    #[test]
    fn test_evaluate_perfect_match() {
        let pipeline = vec![
            make_result("A.JPG", "舗設状況", "No.1", "t=50mm"),
            make_result("B.JPG", "転圧状況", "No.2", ""),
        ];
        let gt = vec![
            make_result("A.JPG", "舗設状況", "No.1", "t=50mm"),
            make_result("B.JPG", "転圧状況", "No.2", ""),
        ];

        let report = evaluate(&pipeline, &gt, &EvalField::default_fields());
        assert_eq!(report.total_photos, 2);
        assert_eq!(report.matched_photos, 2);
        assert_eq!(report.missing_photos, 0);
        assert!(report.diffs.is_empty());
        for fr in &report.field_results {
            assert_eq!(fr.matched, fr.total);
        }
    }

    #[test]
    fn test_evaluate_with_diffs() {
        let pipeline = vec![
            make_result("A.JPG", "舗設状況", "No.1", "t=50mm"),
            make_result("B.JPG", "コアー採取状況", "", "t=50mm"),
        ];
        let gt = vec![
            make_result("A.JPG", "舗設状況", "No.1", "t=50mm"),
            make_result("B.JPG", "コアー厚さ測定", "No.8 右車線", "t=50mm"),
        ];

        let report = evaluate(&pipeline, &gt, &EvalField::default_fields());
        assert_eq!(report.total_photos, 2);
        assert_eq!(report.matched_photos, 1); // A matches all, B doesn't
        assert_eq!(report.diffs.len(), 2); // B: remarks + station

        // remarks: A matches, B doesn't
        let remarks_result = report.field_results.iter().find(|r| r.field == EvalField::Remarks).unwrap();
        assert_eq!(remarks_result.matched, 1);
        assert_eq!(remarks_result.total, 2);
    }

    #[test]
    fn test_evaluate_station_normalization() {
        let pipeline = vec![
            make_result("A.JPG", "", "No.1R", ""),
        ];
        let gt = vec![
            make_result("A.JPG", "", "No.1 右車線", ""),
        ];

        let report = evaluate(&pipeline, &gt, &[EvalField::Station]);
        let station_result = &report.field_results[0];
        assert_eq!(station_result.matched, 1);
        assert_eq!(station_result.total, 1);
    }

    #[test]
    fn test_evaluate_missing_photos() {
        let pipeline = vec![
            make_result("A.JPG", "舗設状況", "No.1", ""),
        ];
        let gt = vec![
            make_result("A.JPG", "舗設状況", "No.1", ""),
            make_result("C.JPG", "転圧状況", "No.2", ""),
        ];

        let report = evaluate(&pipeline, &gt, &EvalField::default_fields());
        assert_eq!(report.total_photos, 2);
        assert_eq!(report.missing_photos, 1);
    }

    #[test]
    fn test_evaluate_empty_filename_skipped() {
        let pipeline = vec![
            make_result("A.JPG", "舗設状況", "No.1", ""),
        ];
        let gt = vec![
            make_result("A.JPG", "舗設状況", "No.1", ""),
            make_result("", "", "", ""), // placeholder row
        ];

        let report = evaluate(&pipeline, &gt, &EvalField::default_fields());
        assert_eq!(report.total_photos, 1); // empty filename skipped
    }

    #[test]
    fn test_evaluate_field_filter() {
        let pipeline = vec![
            make_result("A.JPG", "wrong", "No.1", "wrong"),
        ];
        let gt = vec![
            make_result("A.JPG", "correct", "No.1", "correct"),
        ];

        // station only → should pass
        let report = evaluate(&pipeline, &gt, &[EvalField::Station]);
        assert_eq!(report.field_results.len(), 1);
        assert_eq!(report.field_results[0].matched, 1);
        assert!(report.diffs.is_empty());
    }

    #[test]
    fn test_normalize_station_left_lane() {
        assert_eq!(normalize_station("No.3 左車線"), "No.3L");
        assert_eq!(normalize_station("No.3 L"), "No.3L");
        assert_eq!(normalize_station("No.3 左"), "No.3L");
    }

    #[test]
    fn test_normalize_station_fullwidth() {
        assert_eq!(normalize_station("Ｎｏ．１"), "No.1");
        assert_eq!(normalize_station("Ｎｏ．１Ｒ"), "No.1R");
    }

    #[test]
    fn test_evaluate_empty_fields_uses_default() {
        let pipeline = vec![make_result("A.JPG", "x", "No.1", "")];
        let gt = vec![make_result("A.JPG", "x", "No.1", "")];

        // empty slice → default fields (remarks, station, measurements)
        let report = evaluate(&pipeline, &gt, &[]);
        assert_eq!(report.field_results.len(), 3);
    }

    #[test]
    fn test_field_result_accuracy_zero_total() {
        let fr = FieldResult { field: EvalField::Remarks, total: 0, matched: 0 };
        assert_eq!(fr.accuracy(), 0.0);
    }
}
