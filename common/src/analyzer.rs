//! 解析ロジック（CLI/WASM共通）
//!
//! Step1結果からの工種自動判定と、Step1+Step2結果のマージ処理

use crate::types::{AnalysisResult, RawImageData, Step2Result};
use crate::prompts::PHOTO_CATEGORIES;
use std::collections::{HashMap, HashSet};

/// 画像メタデータ（CLI/WASM共通）
#[derive(Debug, Clone, Default)]
pub struct ImageMeta {
    pub file_name: String,
    pub file_path: String,  // WASMでは空文字可
    pub date: String,       // WASMでは空文字可
}

/// Step1結果から工種を自動判定
/// キーワードマッチングで工種を検出
pub fn detect_work_types(raw_data: &[RawImageData]) -> Vec<String> {
    let mut types = HashSet::new();

    for r in raw_data {
        let cat = r.photo_category.as_str();
        let text = r.detected_text.as_str();
        let scene = r.scene_description.as_str();

        // 舗装工の判定
        if cat.contains("温度") || cat.contains("転圧") || cat.contains("舗設")
            || cat.contains("敷均し") || cat.contains("乳剤") || cat.contains("路盤")
            || text.contains("アスファルト") || scene.contains("アスファルト")
            || scene.contains("フィニッシャー") || scene.contains("ローラー")
        {
            types.insert("舗装工".to_string());
        }

        // 区画線工の判定
        if cat.contains("区画線") || text.contains("区画線") || text.contains("ライン")
            || scene.contains("白線") || scene.contains("区画線")
        {
            types.insert("区画線工".to_string());
        }

        // 構造物撤去工の判定
        if cat.contains("取壊し") || text.contains("撤去") || text.contains("取壊")
            || scene.contains("解体") || scene.contains("撤去")
        {
            types.insert("構造物撤去工".to_string());
        }

        // 道路土工の判定
        if cat.contains("掘削") || cat.contains("路床") || text.contains("掘削")
            || scene.contains("掘削") || scene.contains("バックホウ")
        {
            types.insert("道路土工".to_string());
        }

        // 排水構造物工の判定
        if text.contains("側溝") || text.contains("集水") || text.contains("人孔")
            || scene.contains("側溝") || scene.contains("マンホール")
        {
            types.insert("排水構造物工".to_string());
        }

        // 人孔改良工の判定
        if text.contains("人孔改良") || text.contains("マンホール蓋")
        {
            types.insert("人孔改良工".to_string());
        }
    }

    types.into_iter().collect()
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
    fn test_detect_work_types_pavement() {
        // 舗装工検出
        let raw_data = vec![
            RawImageData {
                file_name: "temp1.jpg".to_string(),
                photo_category: "到着温度".to_string(),
                scene_description: "アスファルト舗装".to_string(),
                ..Default::default()
            },
        ];

        let types = detect_work_types(&raw_data);
        assert!(types.contains(&"舗装工".to_string()));
    }

    #[test]
    fn test_detect_work_types_marking() {
        // 区画線工検出
        let raw_data = vec![
            RawImageData {
                file_name: "line1.jpg".to_string(),
                detected_text: "区画線施工".to_string(),
                scene_description: "白線を引いている".to_string(),
                ..Default::default()
            },
        ];

        let types = detect_work_types(&raw_data);
        assert!(types.contains(&"区画線工".to_string()));
    }

    #[test]
    fn test_detect_work_types_multiple() {
        // 複数工種検出
        let raw_data = vec![
            RawImageData {
                file_name: "temp1.jpg".to_string(),
                photo_category: "転圧状況".to_string(),
                scene_description: "ローラーで転圧".to_string(),
                ..Default::default()
            },
            RawImageData {
                file_name: "line1.jpg".to_string(),
                scene_description: "区画線の白線".to_string(),
                ..Default::default()
            },
            RawImageData {
                file_name: "demolish1.jpg".to_string(),
                photo_category: "取壊し状況".to_string(),
                scene_description: "解体作業".to_string(),
                ..Default::default()
            },
        ];

        let types = detect_work_types(&raw_data);
        assert!(types.contains(&"舗装工".to_string()));
        assert!(types.contains(&"区画線工".to_string()));
        assert!(types.contains(&"構造物撤去工".to_string()));
        assert_eq!(types.len(), 3);
    }

    #[test]
    fn test_detect_work_types_empty() {
        // 該当なし
        let raw_data = vec![
            RawImageData {
                file_name: "other.jpg".to_string(),
                photo_category: "その他".to_string(),
                scene_description: "風景写真".to_string(),
                ..Default::default()
            },
        ];

        let types = detect_work_types(&raw_data);
        assert!(types.is_empty());
    }

    #[test]
    fn test_detect_work_types_drainage() {
        // 排水構造物工検出
        let raw_data = vec![
            RawImageData {
                file_name: "drain1.jpg".to_string(),
                detected_text: "側溝設置".to_string(),
                scene_description: "マンホール".to_string(),
                ..Default::default()
            },
        ];

        let types = detect_work_types(&raw_data);
        assert!(types.contains(&"排水構造物工".to_string()));
    }

    #[test]
    fn test_detect_work_types_manhole() {
        // 人孔改良工検出
        let raw_data = vec![
            RawImageData {
                file_name: "manhole1.jpg".to_string(),
                detected_text: "人孔改良工事".to_string(),
                ..Default::default()
            },
        ];

        let types = detect_work_types(&raw_data);
        assert!(types.contains(&"人孔改良工".to_string()));
    }

    #[test]
    fn test_detect_work_types_earthwork() {
        // 道路土工検出
        let raw_data = vec![
            RawImageData {
                file_name: "earth1.jpg".to_string(),
                photo_category: "掘削状況".to_string(),
                scene_description: "バックホウで掘削".to_string(),
                ..Default::default()
            },
        ];

        let types = detect_work_types(&raw_data);
        assert!(types.contains(&"道路土工".to_string()));
    }

    #[test]
    fn test_merge_results() {
        // マージ結果確認
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

        let step2_results = vec![
            Step2Result {
                file_name: "test1.jpg".to_string(),
                work_type: "舗装工".to_string(),
                variety: "舗装打換え工".to_string(),
                subphase: "表層工".to_string(),
                station: "No.10".to_string(),
                description: "到着温度測定".to_string(),
                reasoning: "温度計測写真".to_string(),
                ..Default::default()
            },
        ];

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

        // test1.jpg: Step2結果あり
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

        // test2.jpg: Step2結果なし（RawImageDataのscene_descriptionを使用）
        let r2 = &results[1];
        assert_eq!(r2.file_name, "test2.jpg");
        assert_eq!(r2.file_path, "/path/to/test2.jpg");
        assert!(!r2.has_board);
        assert_eq!(r2.work_type, ""); // Step2結果なし
        assert_eq!(r2.description, "転圧状況"); // scene_descriptionを使用
    }

    #[test]
    fn test_merge_results_empty_images() {
        // 画像メタデータなしの場合
        let raw_data = vec![
            RawImageData {
                file_name: "test.jpg".to_string(),
                scene_description: "工事写真".to_string(),
                ..Default::default()
            },
        ];

        let step2_results = vec![];
        let images: Vec<ImageMeta> = vec![];

        let results = merge_results(&raw_data, &step2_results, &images);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_name, "test.jpg");
        assert_eq!(results[0].file_path, ""); // 空文字
        assert_eq!(results[0].date, "");      // 空文字
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
