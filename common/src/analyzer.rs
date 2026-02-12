//! 解析ロジック（CLI/WASM共通）
//!
//! Step1結果からの工種自動判定

use crate::types::{RawImageData, WorkTypeDefinition};
use crate::work_type_defs::DEFAULT_WORK_TYPE_DEFINITIONS;
use std::collections::HashSet;
use thiserror::Error;

/// AI解析関連エラー（パース失敗・tagger空結果）
#[derive(Error, Debug)]
pub enum AnalyzerError {
    /// AIレスポンスのパース失敗（再試行可能かの判断に使う）
    #[error("AI response parse error: {0}")]
    ResponseParse(String),

    /// photo-tagger結果が空
    #[error("Photo tagger returned no results")]
    TaggerEmpty,
}

/// Step1結果から工種を自動判定（デフォルト定義使用）
pub fn detect_work_types(raw_data: &[RawImageData]) -> Vec<String> {
    detect_work_types_with(raw_data, DEFAULT_WORK_TYPE_DEFINITIONS)
}

/// Step1結果から工種を自動判定（カスタム定義使用）
pub fn detect_work_types_with(raw_data: &[RawImageData], definitions: &[WorkTypeDefinition]) -> Vec<String> {
    let mut types = HashSet::new();

    for r in raw_data {
        let cat = r.photo_category.to_lowercase();
        let text = r.detected_text.to_lowercase();
        let scene = r.scene_description.to_lowercase();

        for def in definitions {
            let matched = def.category_keywords.iter().any(|kw| cat.contains(&kw.to_lowercase()))
                || def.text_keywords.iter().any(|kw| text.contains(&kw.to_lowercase()))
                || def.scene_keywords.iter().any(|kw| scene.contains(&kw.to_lowercase()));
            if matched {
                types.insert(def.name.to_string());
            }
        }
    }

    types.into_iter().collect()
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

}
