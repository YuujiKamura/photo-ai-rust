//! Step2解析関連
//!
//! - Step2Result: マスタ照合の出力
//! - build_step2_prompt: Step2用プロンプト
//! - parse_step2_response: Step2レスポンスのパース
//! - merge_results: Step1+Step2結果の統合

use crate::error::{Error, Result};
use crate::hierarchy::HierarchyMaster;
use crate::parser::extract_json;
use crate::prompts::PHOTO_CATEGORIES;
use crate::types::{AnalysisResult, RawImageData};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Step2の出力: マスタ照合結果
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Step2Result {
    pub file_name: String,
    pub work_type: String,
    pub variety: String,
    #[serde(alias = "detail")]
    pub subphase: String,
    pub remarks: String,
    pub station: String,
    pub description: String,
    pub reasoning: String,
}

/// Step2レスポンスをパース
///
/// マスタ照合結果（Step2Result配列）をパースする
pub fn parse_step2_response(response: &str) -> Result<Vec<Step2Result>> {
    let json_str = extract_json(response)?;
    let results: Vec<Step2Result> = serde_json::from_str(json_str.trim())
        .map_err(|e| Error::Parse(format!("Step2 JSONパースエラー: {}", e)))?;
    Ok(results)
}

/// Step2プロンプト生成（マスタ照合用）
pub fn build_step2_prompt(raw_data: &[RawImageData], master: &HierarchyMaster) -> String {
    let hierarchy_json = master.to_hierarchy_json();
    let hierarchy_str = serde_json::to_string(&hierarchy_json).unwrap_or_default();

    let raw_data_str = raw_data
        .iter()
        .map(|d| {
            format!(
                r#"
ファイル: {}
黒板: {}
OCRテキスト: {}
数値: {}
シーン: {}
写真区分: {}"#,
                d.file_name,
                if d.has_board { "あり" } else { "なし" },
                if d.detected_text.is_empty() { "なし" } else { &d.detected_text },
                if d.measurements.is_empty() { "なし" } else { &d.measurements },
                d.scene_description,
                d.photo_category
            )
        })
        .collect::<Vec<_>>()
        .join("\n---\n");

    format!(
        r#"あなたは工事写真の分類専門家です。
以下の画像解析結果を、工種マスタに基づいて正確に分類してください。

## 工種マスタ（階層構造）
{hierarchy_str}

## 画像解析結果
{raw_data_str}

## 出力ルール
1. photoCategory は写真種別（マスタの写真種別と一致）を選択
2. workType, variety, subphase は必ずマスタに存在する値を選択
3. 選んだ photoCategory と一致する行の組み合わせのみ使用
4. remarks はマスタの「備考」から選択（該当なしは空文字）
5. 該当なしの場合は空文字
6. 乳剤散布状況と養生砂散布状況の判別: スプレイヤーで乳剤を散布する人と飛散防止のベニヤ板を持って立つ人が並ぶ場合は乳剤散布状況

## 出力形式（JSON配列）
```json
[
  {{
    "fileName": "ファイル名",
    "photoCategory": "写真区分",
    "workType": "工種",
    "variety": "種別",
    "subphase": "作業段階",
    "remarks": "撮影内容（マスタの備考から選択）",
    "station": "測点",
    "description": "写真説明",
    "reasoning": "分類理由"
  }}
]
```

- JSON配列のみ出力。説明文は不要
"#
    )
}

/// 画像メタデータ（CLI/WASM共通）
#[derive(Debug, Clone, Default)]
pub struct ImageMeta {
    pub file_name: String,
    pub file_path: String,  // WASMでは空文字可
    pub date: String,       // WASMでは空文字可
}

/// Step1+Step2結果をマージしてAnalysisResult生成
pub fn merge_results(
    raw_data: &[RawImageData],
    step2_results: &[Step2Result],
    images: &[ImageMeta],
) -> Vec<AnalysisResult> {
    // file_nameからImageMetaを取得するためのマップ
    let info_map: HashMap<&str, &ImageMeta> = images
        .iter()
        .map(|img| (img.file_name.as_str(), img))
        .collect();

    // file_nameからStep2結果を取得するためのマップ
    let step2_map: HashMap<&str, &Step2Result> = step2_results
        .iter()
        .map(|r| (r.file_name.as_str(), r))
        .collect();

    raw_data
        .iter()
        .map(|raw| {
            let img_info = info_map.get(raw.file_name.as_str());
            let step2 = step2_map.get(raw.file_name.as_str());

            let file_path = img_info
                .map(|i| i.file_path.clone())
                .unwrap_or_default();
            let date = img_info
                .map(|i| i.date.clone())
                .unwrap_or_default();

            AnalysisResult {
                file_name: raw.file_name.clone(),
                file_path,
                date,
                has_board: raw.has_board,
                detected_text: raw.detected_text.clone(),
                measurements: raw.measurements.clone(),
                description: step2
                    .map(|s| s.description.clone())
                    .unwrap_or_else(|| raw.scene_description.clone()),
                photo_category: normalize_photo_category(&raw.photo_category),
                work_type: step2.map(|s| s.work_type.clone()).unwrap_or_default(),
                variety: step2.map(|s| s.variety.clone()).unwrap_or_default(),
                subphase: step2.map(|s| s.subphase.clone()).unwrap_or_default(),
                station: step2.map(|s| s.station.clone()).unwrap_or_default(),
                remarks: step2.map(|s| s.remarks.clone()).unwrap_or_default(),
                remarks_candidates: Vec::new(),
                reasoning: step2.map(|s| s.reasoning.clone()).unwrap_or_default(),
                focus_target: String::new(), // TODO: 1ステップ解析では出力される
            }
        })
        .collect()
}

fn normalize_photo_category(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    if PHOTO_CATEGORIES.contains(&value) {
        value.to_string()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step2_result_default() {
        let result = Step2Result::default();
        assert_eq!(result.file_name, "");
        assert_eq!(result.work_type, "");
        assert_eq!(result.variety, "");
    }

    #[test]
    fn test_step2_result_serialize() {
        let result = Step2Result {
            file_name: "test.jpg".to_string(),
            work_type: "舗装工".to_string(),
            variety: "舗装打換え工".to_string(),
            subphase: "表層工".to_string(),
            station: "No.10".to_string(),
            remarks: "備考".to_string(),
            description: "舗設状況".to_string(),
            reasoning: "温度測定写真のため".to_string(),
        };

        let json = serde_json::to_string(&result).expect("シリアライズ失敗");
        assert!(json.contains("\"workType\":\"舗装工\""));
        assert!(json.contains("\"variety\":\"舗装打換え工\""));
        assert!(json.contains("\"subphase\":\"表層工\""));
    }

    #[test]
    fn test_step2_result_deserialize() {
        let json = r#"{
            "fileName": "test.jpg",
            "workType": "区画線工",
            "variety": "区画線工",
            "subphase": "実線",
            "station": "No.5+10.0"
        }"#;

        let result: Step2Result = serde_json::from_str(json).expect("デシリアライズ失敗");
        assert_eq!(result.file_name, "test.jpg");
        assert_eq!(result.work_type, "区画線工");
        assert_eq!(result.variety, "区画線工");
        assert_eq!(result.subphase, "実線");
        assert_eq!(result.station, "No.5+10.0");
        assert_eq!(result.remarks, ""); // デフォルト値
    }

    #[test]
    fn test_parse_step2_response() {
        let response = r#"```json
[
  {
    "fileName": "test.jpg",
    "workType": "舗装工",
    "variety": "舗装打換え工",
    "subphase": "表層工",
    "station": "No.10",
    "description": "舗設状況",
    "reasoning": "温度測定写真のため"
  }
]
```"#;

        let result = parse_step2_response(response).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name, "test.jpg");
        assert_eq!(result[0].work_type, "舗装工");
        assert_eq!(result[0].variety, "舗装打換え工");
        assert_eq!(result[0].subphase, "表層工");
        assert_eq!(result[0].station, "No.10");
        assert_eq!(result[0].description, "舗設状況");
        assert_eq!(result[0].reasoning, "温度測定写真のため");
    }

    #[test]
    fn test_parse_step2_response_minimal() {
        let response = r#"[{"fileName": "test.jpg", "workType": "区画線工"}]"#;

        let result = parse_step2_response(response).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name, "test.jpg");
        assert_eq!(result[0].work_type, "区画線工");
        assert_eq!(result[0].variety, ""); // デフォルト値
        assert_eq!(result[0].subphase, ""); // デフォルト値
    }

    #[test]
    fn test_parse_step2_response_multiple() {
        let response = r#"```json
[
  {"fileName": "img1.jpg", "workType": "舗装工", "variety": "表層工"},
  {"fileName": "img2.jpg", "workType": "区画線工", "variety": "区画線工"}
]
```"#;

        let result = parse_step2_response(response).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].work_type, "舗装工");
        assert_eq!(result[1].work_type, "区画線工");
    }

    #[test]
    fn test_parse_step2_response_error() {
        let response = "Invalid response without JSON";

        let result = parse_step2_response(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_step2_prompt_single_raw_data() {
        let raw_data = vec![RawImageData {
            file_name: "test.jpg".to_string(),
            has_board: true,
            detected_text: "温度 160.4℃".to_string(),
            measurements: "160.4℃".to_string(),
            scene_description: "舗装工事".to_string(),
            photo_category: "到着温度".to_string(),
        }];

        let master = HierarchyMaster::default();
        let prompt = build_step2_prompt(&raw_data, &master);
        assert!(prompt.contains("test.jpg"));
        assert!(prompt.contains("黒板: あり"));
        assert!(prompt.contains("温度 160.4℃"));
        assert!(prompt.contains("到着温度"));
    }

    #[test]
    fn test_build_step2_prompt_no_board() {
        let raw_data = vec![RawImageData {
            file_name: "test.jpg".to_string(),
            has_board: false,
            detected_text: "".to_string(),
            measurements: "".to_string(),
            scene_description: "舗装工事".to_string(),
            photo_category: "".to_string(),
        }];

        let master = HierarchyMaster::default();
        let prompt = build_step2_prompt(&raw_data, &master);
        assert!(prompt.contains("黒板: なし"));
        assert!(prompt.contains("OCRテキスト: なし"));
        assert!(prompt.contains("数値: なし"));
    }

    #[test]
    fn test_build_step2_prompt_empty_fields() {
        let raw_data = vec![RawImageData {
            file_name: "test.jpg".to_string(),
            has_board: false,
            detected_text: "".to_string(),
            measurements: "".to_string(),
            scene_description: "".to_string(),
            photo_category: "".to_string(),
        }];

        let master = HierarchyMaster::default();
        let prompt = build_step2_prompt(&raw_data, &master);
        assert!(prompt.contains("OCRテキスト: なし"));
        assert!(prompt.contains("数値: なし"));
    }

    #[test]
    fn test_build_step2_prompt_multiple_raw_data() {
        let raw_data = vec![
            RawImageData {
                file_name: "photo1.jpg".to_string(),
                scene_description: "シーン1".to_string(),
                ..Default::default()
            },
            RawImageData {
                file_name: "photo2.jpg".to_string(),
                scene_description: "シーン2".to_string(),
                ..Default::default()
            },
        ];

        let master = HierarchyMaster::default();
        let prompt = build_step2_prompt(&raw_data, &master);
        assert!(prompt.contains("photo1.jpg"));
        assert!(prompt.contains("photo2.jpg"));
        assert!(prompt.contains("---"));
    }

    #[test]
    fn test_build_step2_prompt_contains_json_format() {
        let raw_data = vec![RawImageData {
            file_name: "photo1.jpg".to_string(),
            scene_description: "".to_string(),
            ..Default::default()
        }];

        let master = HierarchyMaster::default();
        let prompt = build_step2_prompt(&raw_data, &master);
        assert!(prompt.contains("\"workType\""));
        assert!(prompt.contains("\"variety\""));
        assert!(prompt.contains("\"subphase\""));
        assert!(prompt.contains("\"remarks\""));
        assert!(prompt.contains("\"station\""));
        assert!(prompt.contains("\"description\""));
        assert!(prompt.contains("\"reasoning\""));
    }

    #[test]
    fn test_build_step2_prompt_contains_rules() {
        let raw_data = vec![RawImageData {
            file_name: "photo1.jpg".to_string(),
            scene_description: "".to_string(),
            ..Default::default()
        }];

        let master = HierarchyMaster::default();
        let prompt = build_step2_prompt(&raw_data, &master);
        assert!(prompt.contains("マスタに存在する値を選択"));
        assert!(prompt.contains("該当なしの場合は空文字"));
    }

    #[test]
    fn test_merge_results() {
        let raw_data = vec![
            RawImageData {
                file_name: "test1.jpg".to_string(),
                has_board: true,
                detected_text: "温度 160.4℃".to_string(),
                measurements: "160.4℃".to_string(),
                scene_description: "舗装作業".to_string(),
                photo_category: "到着温度".to_string(),
            },
            RawImageData {
                file_name: "test2.jpg".to_string(),
                has_board: false,
                scene_description: "転圧状況".to_string(),
                photo_category: "転圧状況".to_string(),
                ..Default::default()
            },
        ];

        let step2_results = vec![Step2Result {
            file_name: "test1.jpg".to_string(),
            work_type: "舗装工".to_string(),
            variety: "舗装打換え工".to_string(),
            subphase: "表層工".to_string(),
            station: "No.10".to_string(),
            description: "到着温度測定".to_string(),
            reasoning: "温度計測写真".to_string(),
            ..Default::default()
        }];

        let images = vec![
            ImageMeta {
                file_name: "test1.jpg".to_string(),
                file_path: "/path/to/test1.jpg".to_string(),
                date: "2025-01-18".to_string(),
            },
            ImageMeta {
                file_name: "test2.jpg".to_string(),
                file_path: "/path/to/test2.jpg".to_string(),
                date: "2025-01-18".to_string(),
            },
        ];

        let results = merge_results(&raw_data, &step2_results, &images);

        assert_eq!(results.len(), 2);

        let r1 = &results[0];
        assert_eq!(r1.file_name, "test1.jpg");
        assert_eq!(r1.file_path, "/path/to/test1.jpg");
        assert_eq!(r1.date, "2025-01-18");
        assert!(r1.has_board);
        assert_eq!(r1.detected_text, "温度 160.4℃");
        assert_eq!(r1.work_type, "舗装工");
        assert_eq!(r1.variety, "舗装打換え工");
        assert_eq!(r1.subphase, "表層工");
        assert_eq!(r1.station, "No.10");
        assert_eq!(r1.description, "到着温度測定");
        assert_eq!(r1.reasoning, "温度計測写真");

        let r2 = &results[1];
        assert_eq!(r2.file_name, "test2.jpg");
        assert_eq!(r2.file_path, "/path/to/test2.jpg");
        assert!(!r2.has_board);
        assert_eq!(r2.work_type, "");
        assert_eq!(r2.description, "転圧状況");
    }

    #[test]
    fn test_merge_results_empty_images() {
        let raw_data = vec![RawImageData {
            file_name: "test.jpg".to_string(),
            scene_description: "工事写真".to_string(),
            ..Default::default()
        }];

        let step2_results = vec![];
        let images: Vec<ImageMeta> = vec![];

        let results = merge_results(&raw_data, &step2_results, &images);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_name, "test.jpg");
        assert_eq!(results[0].file_path, "");
        assert_eq!(results[0].date, "");
        assert_eq!(results[0].description, "工事写真");
    }

    #[test]
    fn test_image_meta_default() {
        let meta = ImageMeta::default();
        assert_eq!(meta.file_name, "");
        assert_eq!(meta.file_path, "");
        assert_eq!(meta.date, "");
    }
}
