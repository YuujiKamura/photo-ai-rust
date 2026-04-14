//! 解析結果の型定義
//!
//! CLIとWeb(WASM)で共有される型:
//! - RawImageData: Step1（画像認識）の出力
//! - AnalysisResult: 最終出力

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Normalize EXIF `DateTimeOriginal` format to ISO-ish.
///
/// EXIF spec uses `YYYY:MM:DD HH:MM:SS`. This converts the YMD colons to hyphens,
/// leaving the `HH:MM:SS` time portion untouched. Idempotent for already-normalized
/// input (e.g. `YYYY-MM-DD HH:MM:SS`). Handles `YYYY:MM:DD` date-only strings too.
///
/// Implementation: only the first two `:` (the YMD separators that appear before
/// the first ASCII space, tab, or `T`) are rewritten. Anything after the separator
/// — notably `HH:MM:SS` — is copied verbatim so time-of-day colons survive.
pub fn normalize_exif_datetime(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    // Find first time-separator (space / tab / 'T'). Everything before it is the
    // date portion; everything from it onward is copied verbatim.
    let split_idx = s
        .char_indices()
        .find(|(_, c)| matches!(c, ' ' | '\t' | 'T'))
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    let (date_part, rest) = s.split_at(split_idx);
    // Replace only the first two ':' in the date portion — matches EXIF YMD.
    let rewritten_date = date_part.replacen(':', "-", 2);
    let mut out = String::with_capacity(s.len());
    out.push_str(&rewritten_date);
    out.push_str(rest);
    out
}

/// 工種キーワード定義
#[derive(Debug, Clone)]
pub struct WorkTypeDefinition {
    /// 工種名（例: "舗装工"）
    pub name: &'static str,
    /// photo_category にマッチするキーワード
    pub category_keywords: &'static [&'static str],
    /// detected_text にマッチするキーワード
    pub text_keywords: &'static [&'static str],
    /// scene_description にマッチするキーワード
    pub scene_keywords: &'static [&'static str],
}

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

    #[serde(default)]
    pub focus_target: String,     // 撮影対象（全景/黒板アップ/温度計アップ等）

    /// スキップフラグ（3枚セット超過分をエクスポートから除外）
    #[serde(default, skip_serializing_if = "is_false")]
    pub skip: bool,

    /// photo-taggerのmachine_id由来グループ番号（内部管理用）
    #[serde(default, skip_serializing_if = "is_zero")]
    pub group: u32,

    /// ラベル上書き（例: {"measurements": "測定内容"}）
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub label_overrides: HashMap<String, String>,

    /// 解析実行日時（JSTローカル時刻文字列）
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub analysis_timestamp: String,

    /// 解析時のAI提供元
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub analysis_provider: String,

    /// 解析時の課金系統
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub analysis_billing: String,

    /// 解析時の呼び出し経路
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub analysis_transport: String,

    /// 解析バイナリのコミットID
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub analysis_commit: String,

    /// 解析時に使ったマスタ選択方式（single/all）
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub analysis_master_selection: String,

    /// 解析時に使ったマスタパス
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub analysis_master_path: String,

    /// 解析時に適用した工種スコープ
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub analysis_scope_work_type: String,

    /// 解析時に適用した写真区分スコープ
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub analysis_scope_photo_type: String,

    /// 解析時に適用した種別スコープ
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub analysis_scope_variety: String,
}

fn is_false(v: &bool) -> bool {
    !v
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

impl AnalysisResult {
    /// 機械関連の写真かどうかを判定
    ///
    /// 実体は `crate::domain::policy::is_machinery_related`。
    /// AnalysisResult の public API 互換のために、このメソッド経由の参照も残す。
    pub fn is_machinery_related(&self) -> bool {
        crate::domain::policy::is_machinery_related(&self.remarks)
    }

    /// 写真区分を enum として取得する（未知ラベル・空文字列は `None`）
    ///
    /// 文字列比較の代わりに `match` で網羅性チェック可能にする。
    pub fn photo_category_enum(&self) -> Option<crate::domain::PhotoCategory> {
        crate::domain::PhotoCategory::from_label(&self.photo_category)
    }

    /// フィールドキーに対応するラベルを返す
    ///
    /// 優先順位:
    /// 1. label_overrides に設定があればそれを使う
    /// 2. 機械関連写真の Station は「機種」
    /// 3. LAYOUT_FIELDS のデフォルトラベル
    pub fn get_label_for_field(&self, key: crate::layout::FieldKey) -> &str {
        use crate::layout::FieldKey;
        let key_str = match key {
            FieldKey::Date => "date",
            FieldKey::PhotoCategory => "photoCategory",
            FieldKey::WorkType => "workType",
            FieldKey::Variety => "variety",
            FieldKey::Subphase => "subphase",
            FieldKey::Station => "station",
            FieldKey::Remarks => "remarks",
            FieldKey::Measurements => "measurements",
        };
        if let Some(label) = self.label_overrides.get(key_str) {
            return label.as_str();
        }
        if self.is_machinery_related() && key == FieldKey::Station {
            "機種"
        } else {
            crate::layout::LAYOUT_FIELDS
                .iter()
                .find(|f| f.key == key)
                .map(|f| f.label)
                .unwrap_or("-")
        }
    }
}

/// 区画線工の線種エントリ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineTypeEntry {
    pub name: String,
    pub length_m: f64,
}

/// 区画線工の線種リスト設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineTypesConfig {
    pub line_types: Vec<LineTypeEntry>,
}

/// 写真データのトレイト（異なるAnalysisResult型に対応）
pub trait PhotoData {
    fn file_path(&self) -> &str;
    fn date(&self) -> &str;
    fn photo_category(&self) -> &str;
    fn work_type(&self) -> &str;
    fn variety(&self) -> &str;
    fn subphase(&self) -> &str;
    fn station(&self) -> &str;
    fn remarks(&self) -> &str;
    fn measurements(&self) -> &str;

    /// フィールドキーから値を取得（Excel/PDF共通）
    fn get_field_value(&self, key: crate::layout::FieldKey) -> &str {
        use crate::layout::FieldKey;
        let v = match key {
            FieldKey::Date => self.date(),
            FieldKey::PhotoCategory => self.photo_category(),
            FieldKey::WorkType => self.work_type(),
            FieldKey::Variety => self.variety(),
            FieldKey::Subphase => self.subphase(),
            FieldKey::Station => self.station(),
            FieldKey::Remarks => self.remarks(),
            FieldKey::Measurements => self.measurements(),
        };
        if v.is_empty() { "-" } else { v }
    }

    /// フィールドキーからラベルを取得（デフォルト: LAYOUT_FIELDS定義値）
    fn get_label_for_field(&self, key: crate::layout::FieldKey) -> &str {
        crate::layout::LAYOUT_FIELDS
            .iter()
            .find(|f| f.key == key)
            .map(|f| f.label)
            .unwrap_or("-")
    }
}

impl PhotoData for AnalysisResult {
    fn file_path(&self) -> &str { &self.file_path }
    fn date(&self) -> &str { &self.date }
    fn photo_category(&self) -> &str { &self.photo_category }
    fn work_type(&self) -> &str { &self.work_type }
    fn variety(&self) -> &str { &self.variety }
    fn subphase(&self) -> &str { &self.subphase }
    fn station(&self) -> &str { &self.station }
    fn remarks(&self) -> &str { &self.remarks }
    fn measurements(&self) -> &str { &self.measurements }

    fn get_label_for_field(&self, key: crate::layout::FieldKey) -> &str {
        AnalysisResult::get_label_for_field(self, key)
    }
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
            analysis_timestamp: "2026-04-09 13:00:00 +0900".to_string(),
            analysis_provider: "claude".to_string(),
            analysis_billing: "subscription".to_string(),
            analysis_transport: "agent_api".to_string(),
            analysis_commit: "370fea0cc5b8aa2201bad392c3415b32af78a13a".to_string(),
            analysis_master_selection: "single".to_string(),
            analysis_master_path: "master/by_work_type/舗装工.csv".to_string(),
            analysis_scope_work_type: "舗装工".to_string(),
            analysis_scope_photo_type: "施工状況写真".to_string(),
            analysis_scope_variety: "路面切削工".to_string(),
            ..Default::default()
        };

        let json = serde_json::to_string(&original).expect("シリアライズ失敗");
        let restored: AnalysisResult = serde_json::from_str(&json).expect("デシリアライズ失敗");

        assert_eq!(original.file_name, restored.file_name);
        assert_eq!(original.work_type, restored.work_type);
        assert_eq!(original.has_board, restored.has_board);
        assert_eq!(original.photo_category, restored.photo_category);
        assert_eq!(original.analysis_timestamp, restored.analysis_timestamp);
        assert_eq!(original.analysis_provider, restored.analysis_provider);
        assert_eq!(original.analysis_commit, restored.analysis_commit);
        assert_eq!(original.analysis_master_selection, restored.analysis_master_selection);
        assert_eq!(original.analysis_scope_work_type, restored.analysis_scope_work_type);
    }

    #[test]
    fn test_analysis_metadata_missing_in_json_defaults_empty() {
        let json = r#"{"fileName": "minimal.jpg"}"#;
        let result: AnalysisResult = serde_json::from_str(json).expect("デシリアライズ失敗");
        assert_eq!(result.analysis_timestamp, "");
        assert_eq!(result.analysis_provider, "");
        assert_eq!(result.analysis_billing, "");
        assert_eq!(result.analysis_transport, "");
        assert_eq!(result.analysis_commit, "");
        assert_eq!(result.analysis_master_selection, "");
        assert_eq!(result.analysis_master_path, "");
        assert_eq!(result.analysis_scope_work_type, "");
        assert_eq!(result.analysis_scope_photo_type, "");
        assert_eq!(result.analysis_scope_variety, "");
    }

    // =============================================
    // group フィールドテスト
    // =============================================

    #[test]
    fn test_group_zero_not_serialized() {
        let result = AnalysisResult {
            file_name: "test.jpg".to_string(),
            group: 0,
            ..Default::default()
        };
        let json = serde_json::to_string(&result).expect("シリアライズ失敗");
        assert!(!json.contains("group"), "group:0はJSON出力に現れないべき");
    }

    #[test]
    fn test_group_nonzero_serialized_and_deserialized() {
        let result = AnalysisResult {
            file_name: "test.jpg".to_string(),
            group: 5,
            ..Default::default()
        };
        let json = serde_json::to_string(&result).expect("シリアライズ失敗");
        assert!(json.contains("\"group\":5"), "group:5はJSON出力に現れるべき");

        let restored: AnalysisResult = serde_json::from_str(&json).expect("デシリアライズ失敗");
        assert_eq!(restored.group, 5);
    }

    #[test]
    fn test_group_missing_in_json_defaults_to_zero() {
        let json = r#"{"fileName": "old.jpg"}"#;
        let result: AnalysisResult = serde_json::from_str(json).expect("デシリアライズ失敗");
        assert_eq!(result.group, 0, "JSONにgroupがない場合は0になるべき");
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
            ..Default::default()
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
    // normalize_exif_datetime テスト
    // =============================================

    #[test]
    fn test_normalize_exif_datetime_full_exif_format() {
        // EXIF仕様の YYYY:MM:DD HH:MM:SS 形式。YMDの `:` のみ `-` に置換され、
        // 時刻部分 HH:MM:SS のコロンは保持されることを確認。
        assert_eq!(
            normalize_exif_datetime("2025:12:26 13:47:52"),
            "2025-12-26 13:47:52"
        );
    }

    #[test]
    fn test_normalize_exif_datetime_already_iso_passthrough() {
        // 既にISO形式ならそのまま返す（冪等性）。
        assert_eq!(
            normalize_exif_datetime("2025-12-26 13:47:52"),
            "2025-12-26 13:47:52"
        );
        assert_eq!(normalize_exif_datetime("2025-01-01"), "2025-01-01");
    }

    #[test]
    fn test_normalize_exif_datetime_date_only() {
        // 時刻なしのEXIF日付（bare `YYYY:MM:DD`）。
        assert_eq!(normalize_exif_datetime("2025:12:26"), "2025-12-26");
    }

    #[test]
    fn test_normalize_exif_datetime_mixed_separators() {
        // 既に1つ目が `-` になっているケース、`T` セパレータなど。
        assert_eq!(
            normalize_exif_datetime("2025-12:26 13:47:52"),
            "2025-12-26 13:47:52"
        );
        assert_eq!(
            normalize_exif_datetime("2025:12:26T13:47:52"),
            "2025-12-26T13:47:52"
        );
    }

    #[test]
    fn test_normalize_exif_datetime_empty_string() {
        assert_eq!(normalize_exif_datetime(""), "");
    }

    #[test]
    fn test_normalize_exif_datetime_preserves_time_colons() {
        // 3つ以上の `:` があっても時刻部分はそのまま（replacenでn=2制限）。
        let input = "2025:01:02 03:04:05";
        let out = normalize_exif_datetime(input);
        assert_eq!(out, "2025-01-02 03:04:05");
        // 時刻側コロン2個が維持されていること
        assert_eq!(out.matches(':').count(), 2);
    }
}
