//! 解析結果の型定義
//!
//! CLIとWeb(WASM)で共有される型:
//! - RawImageData: Step1（画像認識）の出力
//! - Step2Result: Step2（マスタ照合）の出力
//! - AnalysisResult: 最終出力（Step1+Step2をマージ）

use serde::{Deserialize, Serialize};

/// Step1の出力: 画像から抽出した生データ
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RawImageData {
    pub file_name: String,
    pub has_board: bool,
    pub detected_text: String,
    pub measurements: String,
    pub scene_description: String,
    pub photo_category: String,
}

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

/// AI解析結果
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResult {
    pub file_name: String,

    /// 画像ファイルの絶対パス（PDF出力時に使用）
    #[serde(default)]
    pub file_path: String,

    /// 撮影日時（EXIF DateTimeOriginal）
    #[serde(default)]
    pub date: String,

    #[serde(default)]
    pub work_type: String,        // 工種

    #[serde(default)]
    pub variety: String,          // 種別

    #[serde(default)]
    #[serde(alias = "detail")]
    pub subphase: String,         // 作業段階

    #[serde(default)]
    pub station: String,          // 測点

    #[serde(default)]
    pub remarks: String,          // 備考

    #[serde(default)]
    pub remarks_candidates: Vec<String>, // 備考候補（AIが提案）

    #[serde(default)]
    pub description: String,      // 写真説明

    #[serde(default)]
    pub has_board: bool,          // 黒板あり

    #[serde(default)]
    pub detected_text: String,    // OCRテキスト

    #[serde(default)]
    pub measurements: String,     // 数値データ

    #[serde(default)]
    pub photo_category: String,   // 写真区分

    #[serde(default)]
    pub reasoning: String,        // 分類理由
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_result_default() {
        let result = AnalysisResult::default();
        assert_eq!(result.file_name, "");
        assert_eq!(result.has_board, false);
    }

    #[test]
    fn test_analysis_result_serialize() {
        let result = AnalysisResult {
            file_name: "test.jpg".to_string(),
            work_type: "舗装工".to_string(),
            variety: "表層工".to_string(),
            has_board: true,
            ..Default::default()
        };

        let json = serde_json::to_string(&result).expect("シリアライズ失敗");
        assert!(json.contains("\"fileName\":\"test.jpg\""));
        assert!(json.contains("\"workType\":\"舗装工\""));
        assert!(json.contains("\"hasBoard\":true"));
    }

    #[test]
    fn test_analysis_result_deserialize() {
        let json = r#"{
            "fileName": "photo.jpg",
            "workType": "区画線工",
            "photoCategory": "施工状況写真",
            "hasBoard": false
        }"#;

        let result: AnalysisResult = serde_json::from_str(json).expect("デシリアライズ失敗");
        assert_eq!(result.file_name, "photo.jpg");
        assert_eq!(result.work_type, "区画線工");
        assert_eq!(result.photo_category, "施工状況写真");
        assert_eq!(result.has_board, false);
    }

    #[test]
    fn test_analysis_result_deserialize_missing_fields() {
        // 必須フィールドのみでデシリアライズできることを確認
        let json = r#"{"fileName": "minimal.jpg"}"#;

        let result: AnalysisResult = serde_json::from_str(json).expect("デシリアライズ失敗");
        assert_eq!(result.file_name, "minimal.jpg");
        assert_eq!(result.work_type, ""); // デフォルト値
        assert_eq!(result.has_board, false); // デフォルト値
    }

    #[test]
    fn test_analysis_result_roundtrip() {
        let original = AnalysisResult {
            file_name: "roundtrip.jpg".to_string(),
            date: "2025-01-18".to_string(),
            work_type: "舗装工".to_string(),
            variety: "舗装打換え工".to_string(),
            subphase: "表層工".to_string(),
            station: "No.10".to_string(),
            remarks: "備考テスト".to_string(),
            description: "説明テスト".to_string(),
            has_board: true,
            detected_text: "黒板テキスト".to_string(),
            measurements: "厚さ50mm".to_string(),
            photo_category: "品質管理写真".to_string(),
            reasoning: "分類理由".to_string(),
            ..Default::default()
        };

        let json = serde_json::to_string(&original).expect("シリアライズ失敗");
        let restored: AnalysisResult = serde_json::from_str(&json).expect("デシリアライズ失敗");

        assert_eq!(original.file_name, restored.file_name);
        assert_eq!(original.work_type, restored.work_type);
        assert_eq!(original.has_board, restored.has_board);
        assert_eq!(original.photo_category, restored.photo_category);
    }

    // =============================================
    // RawImageData テスト
    // =============================================

    #[test]
    fn test_raw_image_data_default() {
        let raw = RawImageData::default();
        assert_eq!(raw.file_name, "");
        assert!(!raw.has_board);
        assert_eq!(raw.detected_text, "");
    }

    #[test]
    fn test_raw_image_data_serialize() {
        let raw = RawImageData {
            file_name: "test.jpg".to_string(),
            has_board: true,
            detected_text: "温度 160.4℃".to_string(),
            measurements: "160.4℃".to_string(),
            scene_description: "アスファルト舗装工事".to_string(),
            photo_category: "到着温度".to_string(),
        };

        let json = serde_json::to_string(&raw).expect("シリアライズ失敗");
        assert!(json.contains("\"fileName\":\"test.jpg\""));
        assert!(json.contains("\"hasBoard\":true"));
        assert!(json.contains("\"detectedText\":\"温度 160.4℃\""));
        assert!(json.contains("\"photoCategory\":\"到着温度\""));
    }

    #[test]
    fn test_raw_image_data_deserialize() {
        let json = r#"{
            "fileName": "photo1.jpg",
            "hasBoard": false,
            "sceneDescription": "道路工事"
        }"#;

        let raw: RawImageData = serde_json::from_str(json).expect("デシリアライズ失敗");
        assert_eq!(raw.file_name, "photo1.jpg");
        assert!(!raw.has_board);
        assert_eq!(raw.scene_description, "道路工事");
        assert_eq!(raw.detected_text, ""); // デフォルト値
    }

    // =============================================
    // Step2Result テスト
    // =============================================

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
}
