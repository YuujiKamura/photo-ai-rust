//! 解析ロジック（CLI/WASM共通）
//!
//! Step1結果からの工種自動判定

use crate::types::RawImageData;
use std::collections::HashSet;

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
