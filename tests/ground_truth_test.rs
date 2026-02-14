//! Ground Truth バリデーションテスト
//!
//! 解析結果JSONと正解データを比較し、回帰を検出する。
//! result JSONが存在しない環境ではスキップする。

use photo_ai_common::AnalysisResult;
use std::collections::HashMap;
use std::path::Path;

/// Ground Truth エントリ（比較対象フィールドのみ）
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroundTruthEntry {
    file_name: String,
    work_type: String,
    variety: String,
    subphase: String,
    station: String,
    remarks: String,
    photo_category: String,
}

const RESULT_PATH: &str = r"H:\マイドライブ\〇市道 南千反畑町第１号線舗装補修工事\１５工事写真\0211切削\施工状況\result_stability_test.json";

#[test]
fn gt_0211_cutting() {
    // result JSONが存在しなければスキップ
    if !Path::new(RESULT_PATH).exists() {
        eprintln!("SKIP: result JSON not found at {}", RESULT_PATH);
        return;
    }

    // Ground Truth 読み込み
    let gt_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("ground_truth")
        .join("0211_cutting.json");
    let gt_data: Vec<GroundTruthEntry> =
        serde_json::from_str(&std::fs::read_to_string(&gt_path).expect("GT JSON読み込み失敗"))
            .expect("GT JSONパース失敗");

    // Result JSON 読み込み
    let result_data: Vec<AnalysisResult> =
        serde_json::from_str(&std::fs::read_to_string(RESULT_PATH).expect("Result JSON読み込み失敗"))
            .expect("Result JSONパース失敗");

    // fileNameでインデックス化
    let result_map: HashMap<&str, &AnalysisResult> = result_data
        .iter()
        .map(|r| (r.file_name.as_str(), r))
        .collect();

    let mut errors: Vec<String> = Vec::new();

    for gt in &gt_data {
        let Some(result) = result_map.get(gt.file_name.as_str()) else {
            errors.push(format!("{}: result JSONに存在しない", gt.file_name));
            continue;
        };

        let fields: &[(&str, &str, &str)] = &[
            ("workType", &gt.work_type, &result.work_type),
            ("variety", &gt.variety, &result.variety),
            ("subphase", &gt.subphase, &result.subphase),
            ("station", &gt.station, &result.station),
            ("remarks", &gt.remarks, &result.remarks),
            ("photoCategory", &gt.photo_category, &result.photo_category),
        ];

        for (field, expected, actual) in fields {
            if expected != actual {
                errors.push(format!(
                    "{} / {}: expected={:?}, actual={:?}",
                    gt.file_name, field, expected, actual
                ));
            }
        }
    }

    if !errors.is_empty() {
        panic!(
            "Ground Truth 不一致 ({} 件):\n  {}",
            errors.len(),
            errors.join("\n  ")
        );
    }
}
