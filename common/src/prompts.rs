//! プロンプト生成モジュール
//!
//! CLIとWeb(WASM)で共有されるプロンプト生成ロジック:
//! - PHOTO_CATEGORIES: 写真区分の定数
//! - build_step1_prompt: Step1（画像認識）用プロンプト
//! - build_step2_prompt: Step2（マスタ照合）用プロンプト

use crate::types::RawImageData;
use crate::hierarchy::HierarchyMaster;

/// 写真区分（工種階層マスタの写真種別）
pub const PHOTO_CATEGORIES: &[&str] = &[
    "使用材料写真",
    "出来形管理写真",
    "品質管理写真",
    "安全管理写真",
    "施工状況写真",
    "着手前及び完成写真",
];

/// Step1プロンプト生成（画像認識用）
///
/// # Arguments
/// * `images` - 画像メタデータのスライス。各要素は (ファイル名, 日付Option)
///
/// # Returns
/// Step1解析用のプロンプト文字列
pub fn build_step1_prompt(images: &[(&str, Option<&str>)]) -> String {
    let photo_list = images
        .iter()
        .map(|(name, date)| {
            format!(
                "- {} (撮影: {})",
                name,
                date.unwrap_or("unknown")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let categories = PHOTO_CATEGORIES.join(", ");

    format!(
        r#"あなたは工事写真帳を作成する現場監督です。複数の写真を同時に解析し、一貫性のある分類を行ってください。

## 写真区分（写真種別）
以下から最も適切なものを選択：
{categories}

## 出力形式（厳密にこのJSON配列形式で出力）
[
  {{
    "fileName": "ファイル名",
    "hasBoard": true/false,
    "detectedText": "黒板・看板から読み取った全テキスト",
    "measurements": "数値データ（温度、寸法、密度等）単位付き",
    "sceneDescription": "写真に写っているものの客観的な説明",
    "photoCategory": "写真区分から選択"
  }}
]

## 注意
- 黒板のテキストは正確にOCR
- 数値は単位も含めて正確に（例: "160.4℃", "厚さ50mm"）
- 同じ場所・同じ作業の写真は一貫した分類を
- 推測せず、見えるものだけを記載
- 写真区分は上記リスト以外を出力しない（該当なしは空文字）
- JSON配列のみ出力。説明文は不要

対象写真:
{photo_list}"#
    )
}

/// Step2プロンプト生成（マスタ照合用）
///
/// # Arguments
/// * `raw_data` - Step1で抽出した生データ
/// * `master` - 階層マスタ
///
/// # Returns
/// Step2照合用のプロンプト文字列
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
1. workType, variety, detail は必ずマスタに存在する値を選択
2. マスタにない値は絶対に使用しない
3. remarks はマスタの「備考」から選択（該当なしは空文字）
4. 該当なしの場合は空文字""

## 出力形式（JSON配列）
```json
[
  {{
    "fileName": "ファイル名",
    "workType": "工種（マスタから選択）",
    "variety": "種別（マスタから選択）",
    "detail": "細別（マスタから選択）",
    "remarks": "備考",
    "station": "測点（黒板から読み取れた場合）",
    "description": "写真の説明",
    "reasoning": "分類理由"
  }}
]
```

出力はJSON配列のみ。説明不要。"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // =============================================
    // PHOTO_CATEGORIES テスト
    // =============================================

    #[test]
    fn test_photo_categories_not_empty() {
        assert!(!PHOTO_CATEGORIES.is_empty());
    }

    #[test]
    #[test]
    fn test_photo_categories_contains_construction() {
        assert!(PHOTO_CATEGORIES.contains(&"施工状況写真"));
    }

    #[test]
    fn test_photo_categories_contains_safety() {
        assert!(PHOTO_CATEGORIES.contains(&"安全管理写真"));
    }

    // =============================================
    // build_step1_prompt テスト
    // =============================================

    #[test]
    fn test_build_step1_prompt_single_image() {
        let images = vec![("test.jpg", Some("2025-01-18"))];
        let prompt = build_step1_prompt(&images);

        assert!(prompt.contains("test.jpg"));
        assert!(prompt.contains("2025-01-18"));
        assert!(prompt.contains("施工状況写真"));
        assert!(prompt.contains("JSON配列のみ出力"));
    }

    #[test]
    fn test_build_step1_prompt_multiple_images() {
        let images = vec![
            ("photo1.jpg", Some("2025-01-18")),
            ("photo2.jpg", Some("2025-01-19")),
            ("photo3.jpg", None),
        ];
        let prompt = build_step1_prompt(&images);

        assert!(prompt.contains("photo1.jpg"));
        assert!(prompt.contains("photo2.jpg"));
        assert!(prompt.contains("photo3.jpg"));
        assert!(prompt.contains("2025-01-18"));
        assert!(prompt.contains("unknown")); // None case
    }

    #[test]
    fn test_build_step1_prompt_contains_categories() {
        let images = vec![("test.jpg", None)];
        let prompt = build_step1_prompt(&images);

        // カテゴリがカンマ区切りで含まれていること
        assert!(prompt.contains("使用材料写真, 出来形管理写真"));
    }

    #[test]
    fn test_build_step1_prompt_contains_json_format() {
        let images = vec![("test.jpg", None)];
        let prompt = build_step1_prompt(&images);

        assert!(prompt.contains("\"fileName\""));
        assert!(prompt.contains("\"hasBoard\""));
        assert!(prompt.contains("\"detectedText\""));
        assert!(prompt.contains("\"measurements\""));
        assert!(prompt.contains("\"sceneDescription\""));
        assert!(prompt.contains("\"photoCategory\""));
    }

    #[test]
    fn test_build_step1_prompt_empty_images() {
        let images: Vec<(&str, Option<&str>)> = vec![];
        let prompt = build_step1_prompt(&images);

        // 空でもプロンプトは生成される
        assert!(prompt.contains("対象写真:"));
        assert!(prompt.contains("施工状況写真"));
    }

    // =============================================
    // build_step2_prompt テスト
    // =============================================

    #[test]
    fn test_build_step2_prompt_single_raw_data() {
        let raw_data = vec![RawImageData {
            file_name: "test.jpg".to_string(),
            has_board: true,
            detected_text: "温度 160.4℃".to_string(),
            measurements: "160.4℃".to_string(),
            scene_description: "アスファルト舗装工事".to_string(),
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
            ..Default::default()
        }];
        let master = HierarchyMaster::default();
        let prompt = build_step2_prompt(&raw_data, &master);

        assert!(prompt.contains("黒板: なし"));
    }

    #[test]
    fn test_build_step2_prompt_empty_fields() {
        let raw_data = vec![RawImageData {
            file_name: "test.jpg".to_string(),
            has_board: false,
            detected_text: "".to_string(),
            measurements: "".to_string(),
            scene_description: "工事現場".to_string(),
            photo_category: "施工状況".to_string(),
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
                has_board: true,
                detected_text: "テスト1".to_string(),
                ..Default::default()
            },
            RawImageData {
                file_name: "photo2.jpg".to_string(),
                has_board: false,
                detected_text: "テスト2".to_string(),
                ..Default::default()
            },
        ];
        let master = HierarchyMaster::default();
        let prompt = build_step2_prompt(&raw_data, &master);

        assert!(prompt.contains("photo1.jpg"));
        assert!(prompt.contains("photo2.jpg"));
        assert!(prompt.contains("---")); // 区切り
    }

    #[test]
    fn test_build_step2_prompt_contains_json_format() {
        let raw_data = vec![RawImageData::default()];
        let master = HierarchyMaster::default();
        let prompt = build_step2_prompt(&raw_data, &master);

        assert!(prompt.contains("\"workType\""));
        assert!(prompt.contains("\"variety\""));
        assert!(prompt.contains("\"detail\""));
        assert!(prompt.contains("\"remarks\""));
        assert!(prompt.contains("\"station\""));
        assert!(prompt.contains("\"description\""));
        assert!(prompt.contains("\"reasoning\""));
    }

    #[test]
    fn test_build_step2_prompt_contains_rules() {
        let raw_data = vec![RawImageData::default()];
        let master = HierarchyMaster::default();
        let prompt = build_step2_prompt(&raw_data, &master);

        assert!(prompt.contains("マスタに存在する値を選択"));
        assert!(prompt.contains("マスタにない値は絶対に使用しない"));
        assert!(prompt.contains("該当なしの場合は空文字"));
    }
}
