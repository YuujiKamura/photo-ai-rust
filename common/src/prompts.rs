//! プロンプト生成モジュール
//!
//! CLIとWeb(WASM)で共有されるプロンプト生成ロジック:
//! - PHOTO_CATEGORIES: 写真区分の定数
//! - build_step1_prompt: Step1（画像認識）用プロンプト
//! - build_prompt_for_category: 写真区分別プロンプト（メインAPI）
//! - build_single_step_prompt: 汎用プロンプト（フォールバック）

use crate::hierarchy::HierarchyMaster;

/// 写真区分（工種階層マスタの写真種別）
pub const PHOTO_CATEGORIES: &[&str] = &[
    "使用材料写真",
    "出来形管理写真",
    "品質管理写真",
    "安全管理写真",
    "施工状況写真",
    "着手前及び完成写真",
    "その他",
];

/// Step1プロンプト生成（画像認識用）
pub fn build_step1_prompt(images: &[(&str, Option<&str>)]) -> String {
    let photo_list = format_photo_list(images);
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
    "measurements": "数値と単位のみ（例: 50mm, 160.4℃）注釈・説明不要",
    "sceneDescription": "写真に写っているものの客観的な説明",
    "photoCategory": "写真区分から選択"
  }}
]

## 温度写真の解析（重要）
温度計が写っている写真では、必ず温度計の表示を正確に読み取ってください：
- デジタル温度計の液晶表示、または棒状温度計の目盛りを確認
- measurements に実測値を記録（例: "161.1℃", "32.6℃"）
- よくある誤読: "32.6℃" を "126℃" と読み間違えない（小数点と桁数を確認）
- 温度計の数字が正立・倒立・反転している場合があるので注意

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

// =========================================================================
// 写真区分別プロンプト（メインAPI）
// =========================================================================

/// 写真区分別プロンプト生成（ディスパッチ関数）
///
/// photo_category が指定されていれば区分別プロンプトを使用。
/// 未指定ならば従来の汎用プロンプトにフォールバック。
pub fn build_prompt_for_category(
    images: &[(&str, Option<&str>)],
    master: &HierarchyMaster,
    work_type: Option<&str>,
    variety: Option<&str>,
    photo_category: Option<&str>,
) -> String {
    match photo_category {
        Some("品質管理写真") => build_quality_prompt(images, master, work_type),
        Some("施工状況写真") => build_construction_prompt(images, master, work_type, variety),
        Some("安全管理写真") => build_safety_prompt(images, master),
        Some("出来形管理写真") => build_measurement_prompt(images, master, work_type),
        Some("使用材料写真") => build_material_prompt(images, master, work_type),
        Some("着手前及び完成写真") => build_before_after_prompt(images, master, work_type),
        Some("その他") => build_other_prompt(images, master, work_type),
        _ => build_generic_prompt(images, master, work_type, variety),
    }
}

// =========================================================================
// 共通パーツ
// =========================================================================

fn format_photo_list(images: &[(&str, Option<&str>)]) -> String {
    images
        .iter()
        .map(|(name, date)| {
            format!("- {} (撮影: {})", name, date.unwrap_or("unknown"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn json_output_format() -> &'static str {
    r#"## 出力形式（厳密にこのJSON配列形式で出力）
[
  {
    "fileName": "ファイル名",
    "hasBoard": true/false,
    "detectedText": "黒板・看板から読み取った全テキスト",
    "measurements": "数値と単位のみ（例: 50mm, 160.4℃）注釈・説明不要",
    "description": "写真の説明",
    "photoCategory": "写真区分（固定値）",
    "station": "測点（黒板から読み取れた場合）",
    "remarks": "撮影内容（マスタの備考から1つ選択）",
    "remarksCandidates": ["備考候補1", "備考候補2", "備考候補3"],
    "reasoning": "remarks を選んだ根拠（1〜2文）",
    "focusTarget": "撮影対象（全景/黒板アップ/温度計アップ等）"
  }
]"#
}

fn common_rules() -> &'static str {
    r#"## 共通ルール
- 黒板のテキストは正確にOCR
- 数値は単位も含めて正確に
- JSON配列のみ出力。説明文は不要
- remarks は空にせず、必ずマスタの備考から選択
- remarksCandidates はマスタの備考から候補を3つ挙げる
- reasoning は remarks を選んだ根拠を1〜2文で書く"#
}

fn focus_target_rules() -> &'static str {
    r#"## focusTarget の判定基準
- **全景**: 作業現場全体、重機・車両・作業員が写っている広い構図
- **黒板アップ**: 黒板が画面の大部分を占め、文字が読める状態
- **温度計アップ**: 温度計の表示部分がクローズアップされている
- **その他**: 上記に該当しない場合（材料写真、計器アップ等）"#
}

fn work_type_hint(work_type: Option<&str>, variety: Option<&str>) -> String {
    match work_type {
        Some(wt) => {
            let v_hint = variety
                .map(|v| format!("\n- 種別は「{}」が基本", v))
                .unwrap_or_default();
            format!("- 工種は基本「{}」{}", wt, v_hint)
        }
        None => "- 工種はマスタの候補から選択（該当なしなら空文字）".to_string(),
    }
}

// =========================================================================
// 品質管理写真（温度管理）
// =========================================================================

fn build_quality_prompt(
    images: &[(&str, Option<&str>)],
    master: &HierarchyMaster,
    work_type: Option<&str>,
) -> String {
    let photo_list = format_photo_list(images);
    let filtered = master.filter_by_photo_type("品質管理写真");
    let hierarchy_text = filtered.to_compact_text();
    let wt_hint = work_type_hint(work_type, None);
    let json_fmt = json_output_format();
    let common = common_rules();
    let focus = focus_target_rules();

    format!(
        r#"あなたは工事写真帳を作成する現場監督です。品質管理写真を解析してください。

## 写真区分
photoCategory は「品質管理写真」固定です。

## マスタ候補
{hierarchy_text}

## 制約
{wt_hint}
- 撮影内容（備考）はマスタから選択

## 温度写真サイクル
1台の合材につき、以下の順序で3種類の温度を測定（各3枚 = 計9枚）：
1. **到着温度**: ダンプ到着時（全景・ボードアップ・温度計アップ）
2. **敷均し温度**: フィニッシャー直後（全景・ボードアップ・温度計アップ）
3. **初期締固め前温度**: ローラー転圧前（全景・ボードアップ・温度計アップ）
最後に1日1回：
4. **開放温度**: 交通開放前（全景・ボードアップ・温度計アップ）

## 黒板に複数温度がある場合の判断
黒板に到着温度・敷均し温度・初期締固め前温度が並んで書かれている場合、**値が記入済みの温度のうち、最後のもの**を選ぶ：
- 到着温度だけ記入済み（敷均し℃、初期締固前℃が空欄）→ 到着温度
- 到着温度＋敷均し温度が記入済み（初期締固前℃が空欄）→ 敷均し温度
- 全て記入済み → 初期締固め前温度

## 出力ルール（重要）
- **remarks**: マスタから温度種別を選択（到着温度/敷均し温度/初期締固め前温度/開放温度）
- **measurements**: 該当する温度値のみ（例: "149.6℃"）
- **禁止**: 「温度管理」「温度測定」だけの出力は禁止。必ず具体的な温度種別を選ぶ
- **禁止**: 複数の温度を列挙しない（例: ×「到着160.7℃、敷均し155.4℃」→ ○「155.4℃」）

{json_fmt}

{focus}

{common}

対象写真:
{photo_list}"#
    )
}

// =========================================================================
// 施工状況写真
// =========================================================================

fn build_construction_prompt(
    images: &[(&str, Option<&str>)],
    master: &HierarchyMaster,
    work_type: Option<&str>,
    variety: Option<&str>,
) -> String {
    let photo_list = format_photo_list(images);
    let filtered = master.filter_by_photo_type("施工状況写真");
    let hierarchy_text = filtered.to_compact_text();
    let wt_hint = work_type_hint(work_type, variety);
    let json_fmt = json_output_format();
    let common = common_rules();
    let focus = focus_target_rules();

    format!(
        r#"あなたは工事写真帳を作成する現場監督です。施工状況写真を解析してください。

## 写真区分
photoCategory は「施工状況写真」固定です。

## マスタ候補
{hierarchy_text}

## 制約
{wt_hint}
- 撮影内容（備考）はマスタから選択

## 施工状況写真の判定ポイント
- 乳剤散布状況と養生砂散布状況の判別: スプレイヤーで乳剤を散布する人と飛散防止のベニヤ板を持って立つ人が並ぶ場合は乳剤散布状況
- 処分関連（アスガラ処分）: 黒板に「処分状況」と書かれていれば「アスファルト塊処分状況」、許可票が写っていれば「As塊処分施設許可票」、計量台の上なら「アスファルト塊計量状況」

{json_fmt}

{focus}

{common}

対象写真:
{photo_list}"#
    )
}

// =========================================================================
// 安全管理写真
// =========================================================================

fn build_safety_prompt(
    images: &[(&str, Option<&str>)],
    master: &HierarchyMaster,
) -> String {
    let photo_list = format_photo_list(images);
    let filtered = master.filter_by_photo_type("安全管理写真");
    let hierarchy_text = filtered.to_compact_text();
    let json_fmt = json_output_format();
    let common = common_rules();
    let focus = focus_target_rules();

    format!(
        r#"あなたは工事写真帳を作成する現場監督です。安全管理写真を解析してください。

## 写真区分
photoCategory は「安全管理写真」固定です。

## マスタ候補
{hierarchy_text}

## 制約
- 安全管理写真では工種・種別・細別は空
- 撮影内容（備考）はマスタから選択（朝礼、KY活動、新規入場者教育、重機始業前点検、安全パトロール、店社安全パトロール、社外安全パトロール等）
- 黒板の記載内容から適切な備考を判断

{json_fmt}

{focus}

{common}

対象写真:
{photo_list}"#
    )
}

// =========================================================================
// 出来形管理写真
// =========================================================================

fn build_measurement_prompt(
    images: &[(&str, Option<&str>)],
    master: &HierarchyMaster,
    work_type: Option<&str>,
) -> String {
    let photo_list = format_photo_list(images);
    let filtered = master.filter_by_photo_type("出来形管理写真");
    let hierarchy_text = filtered.to_compact_text();
    let wt_hint = work_type_hint(work_type, None);
    let json_fmt = json_output_format();
    let common = common_rules();
    let focus = focus_target_rules();

    format!(
        r#"あなたは工事写真帳を作成する現場監督です。出来形管理写真を解析してください。

## 写真区分
photoCategory は「出来形管理写真」固定です。

## マスタ候補
{hierarchy_text}

## 制約
{wt_hint}
- 撮影内容（備考）はマスタから選択
- 寸法測定値はmeasurementsに記録（例: "50mm", "厚さ45mm"）
- 出来形は3枚1組（全景・管理値・接写）のことが多い

{json_fmt}

{focus}

{common}

対象写真:
{photo_list}"#
    )
}

// =========================================================================
// 使用材料写真
// =========================================================================

fn build_material_prompt(
    images: &[(&str, Option<&str>)],
    master: &HierarchyMaster,
    work_type: Option<&str>,
) -> String {
    let photo_list = format_photo_list(images);
    let filtered = master.filter_by_photo_type("使用材料写真");
    let hierarchy_text = filtered.to_compact_text();
    let wt_hint = work_type_hint(work_type, None);
    let json_fmt = json_output_format();
    let common = common_rules();
    let focus = focus_target_rules();

    format!(
        r#"あなたは工事写真帳を作成する現場監督です。使用材料写真を解析してください。

## 写真区分
photoCategory は「使用材料写真」固定です。

## マスタ候補
{hierarchy_text}

## 制約
{wt_hint}
- 撮影内容（備考）はマスタから選択（材料検収状況、搬入状況等）

{json_fmt}

{focus}

{common}

対象写真:
{photo_list}"#
    )
}

// =========================================================================
// 着手前及び完成写真
// =========================================================================

fn build_before_after_prompt(
    images: &[(&str, Option<&str>)],
    master: &HierarchyMaster,
    work_type: Option<&str>,
) -> String {
    let photo_list = format_photo_list(images);
    let filtered = master.filter_by_photo_type("着手前及び完成写真");
    let hierarchy_text = filtered.to_compact_text();
    let wt_hint = work_type_hint(work_type, None);
    let json_fmt = json_output_format();
    let common = common_rules();
    let focus = focus_target_rules();

    format!(
        r#"あなたは工事写真帳を作成する現場監督です。着手前及び完成写真を解析してください。

## 写真区分
photoCategory は「着手前及び完成写真」固定です。

## マスタ候補
{hierarchy_text}

## 制約
{wt_hint}
- 着手前・完了・竣工のいずれかを判断
- 撮影内容（備考）はマスタから選択

{json_fmt}

{focus}

{common}

対象写真:
{photo_list}"#
    )
}

// =========================================================================
// その他（使用機械等）
// =========================================================================

fn build_other_prompt(
    images: &[(&str, Option<&str>)],
    master: &HierarchyMaster,
    work_type: Option<&str>,
) -> String {
    let photo_list = format_photo_list(images);
    let filtered = master.filter_by_photo_type("その他");
    let hierarchy_text = filtered.to_compact_text();
    let wt_hint = work_type_hint(work_type, None);
    let json_fmt = json_output_format();
    let common = common_rules();
    let focus = focus_target_rules();

    format!(
        r#"あなたは工事写真帳を作成する現場監督です。その他の写真（使用機械等）を解析してください。

## 写真区分
photoCategory は「その他」固定です。

## マスタ候補
{hierarchy_text}

## 制約
{wt_hint}
- 使用機械写真では工種欄が空白でもよい
- 黒板に「使用機械」と書かれている場合、remarks は「使用機械」

{json_fmt}

{focus}

{common}

対象写真:
{photo_list}"#
    )
}

// =========================================================================
// 汎用プロンプト（フォールバック）
// =========================================================================

/// 写真区分不明時の汎用プロンプト（内部用）
fn build_generic_prompt(
    images: &[(&str, Option<&str>)],
    master: &HierarchyMaster,
    work_type: Option<&str>,
    variety: Option<&str>,
) -> String {
    let photo_list = format_photo_list(images);
    let categories = PHOTO_CATEGORIES.join(", ");
    let hierarchy_text = master.to_compact_text();

    let intro = match work_type {
        Some(wt) => format!("あなたは工事写真帳を作成する現場監督です。工種「{}」の写真を解析してください。", wt),
        _ => "あなたは工事写真帳を作成する現場監督です。写真を解析してください。".to_string(),
    };

    let work_type_constraint = match work_type {
        Some(wt) => {
            let variety_hint = variety
                .map(|v| format!("\n- 種別は「{}」が基本（確実でない場合は他を選択可）", v))
                .unwrap_or_default();
            format!("- 工種は基本「{}」{}", wt, variety_hint)
        }
        _ => "- 工種はマスタの候補から選択（該当なしなら空文字）".to_string(),
    };

    let json_fmt = json_output_format();
    let common = common_rules();
    let focus = focus_target_rules();

    format!(
        r#"{intro}

## 写真区分（写真種別）
以下から最も適切なものを選択：
{categories}

## 工種マスタ（候補一覧）
各行は「写真種別 > 工種 > 種別 > 細別: 備考1, 備考2, ...」の形式です。
備考（撮影内容）からマスタの候補を1つ選んでください。
{hierarchy_text}

## 制約
{work_type_constraint}
- ただし、使用機械写真や安全管理写真では工種が空になることがある（異常ではない）
- 撮影内容（備考）だけをマスタから選択（判断不可なら空文字）
- 上位階層はシステム側で自動決定するため、workType/variety/subphase は空文字でよい

## 使用機械写真の判定
- 黒板に「使用機械」と書かれている場合、photoCategory は「その他」、remarks は「使用機械」とする
- 使用機械写真では工種欄が空白でもよい

## 安全管理写真の判定
- 黒板や写真内容から安全管理（朝礼、KY活動、新規入場者教育、重機始業前点検、安全パトロール等）と判断された場合、photoCategory は「安全管理写真」
- 安全管理写真では工種は空

{json_fmt}

{focus}

## 品質管理写真（温度管理）の解析ルール

### 温度写真サイクル
1台の合材につき、以下の順序で3種類の温度を測定（各3枚 = 計9枚）：
1. **到着温度**: ダンプ到着時（全景・ボードアップ・温度計アップ）
2. **敷均し温度**: フィニッシャー直後（全景・ボードアップ・温度計アップ）
3. **初期締固め前温度**: ローラー転圧前（全景・ボードアップ・温度計アップ）
最後に1日1回：
4. **開放温度**: 交通開放前（全景・ボードアップ・温度計アップ）

### 黒板に複数温度がある場合の判断
黒板に到着温度・敷均し温度・初期締固め前温度が並んで書かれている場合、**値が記入済みの温度のうち、最後のもの**を選ぶ：
- 到着温度だけ記入済み（敷均し℃、初期締固前℃が空欄）→ 到着温度
- 到着温度＋敷均し温度が記入済み（初期締固前℃が空欄）→ 敷均し温度
- 到着温度＋敷均し温度＋初期締固め前温度が記入済み → 初期締固め前温度

### 出力ルール（重要）
- **remarks**: マスタから温度種別を選択（到着温度/敷均し温度/初期締固め前温度/開放温度）
- **measurements**: 該当する温度値のみ（例: "149.6℃"）
- **禁止**: 「温度管理」「温度測定」「アスファルト混合物温度測定」だけの出力は禁止。必ず具体的な温度種別を選ぶ
- **禁止**: 複数の温度を列挙しない（例: ×「到着160.7℃、敷均し155.4℃」→ ○「155.4℃」）

{common}
- 乳剤散布状況と養生砂散布状況の判別: スプレイヤーで乳剤を散布する人と飛散防止のベニヤ板を持って立つ人が並ぶ場合は乳剤散布状況
- 処分関連（アスガラ処分）: 黒板に「処分状況」と書かれていれば「アスファルト塊処分状況」、許可票が写っていれば「As塊処分施設許可票」、計量台の上なら「アスファルト塊計量状況」

対象写真:
{photo_list}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_photo_categories_contains_other() {
        assert!(PHOTO_CATEGORIES.contains(&"その他"));
    }

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
        assert!(prompt.contains("unknown"));
    }

    #[test]
    fn test_build_step1_prompt_contains_categories() {
        let images = vec![("test.jpg", None)];
        let prompt = build_step1_prompt(&images);
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
        assert!(prompt.contains("対象写真:"));
        assert!(prompt.contains("施工状況写真"));
    }

    #[test]
    fn test_build_generic_prompt_contains_compact_text() {
        let csv = r#"写真区分,写真種別,工種,種別,細別,撮影内容,検索パターン
"直接工事費","施工状況写真","舗装工","舗装打換え工","表層工","舗設状況",""
"直接工事費","施工状況写真","舗装工","舗装打換え工","表層工","初期転圧状況",""
"直接工事費","その他","舗装工","","","使用機械",""
"現場管理費","安全管理写真","","","","朝礼",""
"#;
        let master = HierarchyMaster::from_csv_str(csv).unwrap();
        let images = vec![("test.jpg", Some("2025-01-18"))];
        // photo_category=None → build_generic_prompt
        let prompt = build_prompt_for_category(&images, &master, Some("舗装工"), None, None);
        assert!(prompt.contains("舗設状況, 初期転圧状況"));
        assert!(prompt.contains("使用機械"));
        assert!(prompt.contains("安全管理写真"));
    }

    #[test]
    fn test_build_prompt_for_category_quality() {
        let csv = r#"写真区分,写真種別,工種,種別,細別,撮影内容,検索パターン
"直接工事費","品質管理写真","舗装工","舗装打換え工","表層工","到着温度","温度管理\|到着温度"
"直接工事費","品質管理写真","舗装工","舗装打換え工","表層工","敷均し温度",""
"#;
        let master = HierarchyMaster::from_csv_str(csv).unwrap();
        let images = vec![("test.jpg", None)];
        let prompt = build_prompt_for_category(&images, &master, Some("舗装工"), None, Some("品質管理写真"));
        assert!(prompt.contains("品質管理写真"));
        assert!(prompt.contains("到着温度"));
        assert!(prompt.contains("温度写真サイクル"));
        // 施工状況写真固有のルールは含まれない
        assert!(!prompt.contains("乳剤散布状況"));
    }

    #[test]
    fn test_build_prompt_for_category_safety() {
        let csv = r#"写真区分,写真種別,工種,種別,細別,撮影内容,検索パターン
"現場管理費","安全管理写真","","","","朝礼",""
"現場管理費","安全管理写真","","","","社外安全パトロール",""
"#;
        let master = HierarchyMaster::from_csv_str(csv).unwrap();
        let images = vec![("test.jpg", None)];
        let prompt = build_prompt_for_category(&images, &master, None, None, Some("安全管理写真"));
        assert!(prompt.contains("安全管理写真"));
        assert!(prompt.contains("社外安全パトロール"));
        // 温度ルールは含まれない
        assert!(!prompt.contains("温度写真サイクル"));
    }

    #[test]
    fn test_build_prompt_for_category_fallback() {
        let csv = r#"写真区分,写真種別,工種,種別,細別,撮影内容,検索パターン
"直接工事費","施工状況写真","舗装工","舗装打換え工","表層工","舗設状況",""
"#;
        let master = HierarchyMaster::from_csv_str(csv).unwrap();
        let images = vec![("test.jpg", None)];
        // photo_category = None → フォールバック
        let prompt = build_prompt_for_category(&images, &master, None, None, None);
        assert!(prompt.contains("写真区分（写真種別）"));
        assert!(prompt.contains("使用材料写真, 出来形管理写真"));
    }
}
