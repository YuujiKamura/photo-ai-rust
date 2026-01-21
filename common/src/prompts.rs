//! プロンプト生成モジュール
//!
//! CLIとWeb(WASM)で共有されるプロンプト生成ロジック:
//! - PHOTO_CATEGORIES: 写真区分の定数
//! - build_step1_prompt: Step1（画像認識）用プロンプト
//! - build_single_step_prompt: 1ステップ解析用プロンプト

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
- 乳剤散布状況と養生砂散布状況の判別: スプレイヤーで乳剤を散布する人と飛散防止のベニヤ板を持って立つ人が並ぶ場合は乳剤散布状況
- 処分関連の写真（アスガラ処分）: 処分施設、許可票、計量、処分状況を区別
- 黒板に「処分状況」等が書いてあれば、そのテキストを優先
- 写真区分は上記リスト以外を出力しない（該当なしは空文字）
- JSON配列のみ出力。説明文は不要

対象写真:
{photo_list}"#
    )
}

/// 1ステップ解析プロンプト生成（工種指定版）
///
/// 工種が既知の場合、画像認識と分類を1回のAI呼び出しで実行
///
/// # Arguments
/// * `images` - 画像メタデータ
/// * `master` - フィルタ済み階層マスタ（指定工種のみ）
/// * `work_type` - 指定された工種
/// * `variety` - 指定された種別（オプション）
///
/// # Returns
/// 1ステップ解析用のプロンプト文字列
pub fn build_single_step_prompt(
    images: &[(&str, Option<&str>)],
    master: &HierarchyMaster,
    work_type: &str,
    variety: Option<&str>,
) -> String {
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
    let hierarchy_json = master.to_chain_records_json();
    let hierarchy_str = serde_json::to_string(&hierarchy_json).unwrap_or_default();

    let variety_hint = variety
        .map(|v| format!("\n- 種別は「{}」が基本（確実でない場合は他を選択可）", v))
        .unwrap_or_default();

    format!(
        r#"あなたは工事写真帳を作成する現場監督です。工種「{work_type}」の写真を解析してください。

## 写真区分（写真種別）
以下から最も適切なものを選択：
{categories}

## 工種マスタ（チェーンレコード）
{hierarchy_str}

## 階層の意味（重要）
- photoDivision: 写真区分（直接工事費など）
- photoType: 写真種別（施工状況写真、品質管理写真など）
- workType: 工種
- variety: 種別
- subphase: 作業段階
- remarks: 撮影内容（最下層。ここだけを選ぶ）
- patterns: 備考に紐づく検索パターン

## 制約
- 工種は「{work_type}」固定{variety_hint}
- 撮影内容（備考）だけをマスタから選択（判断不可なら空文字）
- 上位階層はシステム側で自動決定するため、workType/variety/subphase は空文字でよい

## 出力形式（厳密にこのJSON配列形式で出力）
[
  {{
    "fileName": "ファイル名",
    "hasBoard": true/false,
    "detectedText": "黒板・看板から読み取った全テキスト",
    "measurements": "数値データ（温度、寸法等）単位付き",
    "description": "写真の説明",
    "photoCategory": "写真区分から選択",
    "station": "測点（黒板から読み取れた場合）",
    "remarks": "撮影内容（マスタの備考から1つ選択）",
    "remarksCandidates": ["備考候補1", "備考候補2", "備考候補3"],
    "reasoning": "remarks を選んだ根拠（OCR/説明のどこが一致したかを短く）"
  }}
]

## 注意
- 黒板のテキストは正確にOCR
- 数値は単位も含めて正確に
- JSON配列のみ出力。説明文は不要
- remarks は空にせず、必ずマスタの備考から選択
- remarksCandidates はマスタの備考から候補を3つ挙げ、すべて remarks と同じ「備考」カテゴリにする
- reasoning は remarks を選んだ根拠を1〜2文で書く
- 乳剤散布状況と養生砂散布状況の判別: スプレイヤーで乳剤を散布する人と飛散防止のベニヤ板を持って立つ人が並ぶ場合は乳剤散布状況
- 処分関連（アスガラ処分）: 黒板に「処分状況」と書かれていれば「アスファルト塊処分状況」、許可票が写っていれば「As塊処分施設許可票」、計量台の上なら「アスファルト塊計量状況」

対象写真:
{photo_list}"#
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

}
