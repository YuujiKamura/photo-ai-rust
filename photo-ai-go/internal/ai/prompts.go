package ai

import (
	"fmt"
	"strings"
)

// PhotoCategories is the fixed list of photo category names (写真区分).
var PhotoCategories = []string{
	"使用材料写真",
	"出来形管理写真",
	"品質管理写真",
	"安全管理写真",
	"施工状況写真",
	"着手前及び完成写真",
	"その他",
}

// ImageMeta holds per-image metadata for prompt generation.
type ImageMeta struct {
	FileName string
	Date     string // empty string means unknown
}

// HierarchyRow is a single row from the master CSV, used for prompt generation.
// This mirrors the subset of HierarchyMaster data needed by the AI prompts.
type HierarchyRow struct {
	PhotoType string // 写真区分
	WorkType  string // 工種
	Variety   string // 種別
	Subphase  string // 細別
	Remarks   string // 備考/撮影内容
}

// BuildStep1Prompt generates the Step1 image recognition prompt.
// Ported from prompts.rs: build_step1_prompt.
func BuildStep1Prompt(images []ImageMeta) string {
	photoList := formatPhotoList(images)
	categories := strings.Join(PhotoCategories, ", ")

	return fmt.Sprintf(
		`あなたは工事写真帳を作成する現場監督です。複数の写真を同時に解析し、一貫性のある分類を行ってください。

## 写真区分（写真種別）
以下から最も適切なものを選択：
%s

## 出力形式（厳密にこのJSON配列形式で出力）
[
  {
    "fileName": "ファイル名",
    "hasBoard": true/false,
    "detectedText": "黒板・看板から読み取った全テキスト",
    "measurements": "数値と単位のみ（例: 50mm, 160.4℃）注釈・説明不要",
    "sceneDescription": "写真に写っているものの客観的な説明",
    "photoCategory": "写真区分から選択"
  }
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

JSON配列のみ出力。説明文不要。

対象写真:
%s`,
		categories, photoList)
}

// BuildPromptForCategory generates the appropriate prompt based on photo category.
// Ported from prompts.rs: build_prompt_for_category.
func BuildPromptForCategory(images []ImageMeta, rows []HierarchyRow, workType, variety, photoCategory string) string {
	switch photoCategory {
	case "品質管理写真":
		return buildQualityPrompt(images, rows, workType)
	case "施工状況写真":
		return buildConstructionPrompt(images, rows, workType, variety)
	case "安全管理写真":
		return buildSafetyPrompt(images, rows)
	case "出来形管理写真":
		return buildMeasurementPrompt(images, rows, workType)
	case "使用材料写真":
		return buildMaterialPrompt(images, rows, workType)
	case "着手前及び完成写真":
		return buildBeforeAfterPrompt(images, rows, workType)
	case "その他":
		return buildOtherPrompt(images, rows, workType)
	default:
		return buildGenericPrompt(images, rows, workType, variety)
	}
}

// ---- shared helpers ----

func formatPhotoList(images []ImageMeta) string {
	parts := make([]string, len(images))
	for i, img := range images {
		date := img.Date
		if date == "" {
			date = "unknown"
		}
		parts[i] = fmt.Sprintf("- %s (撮影: %s)", img.FileName, date)
	}
	return strings.Join(parts, "\n")
}

func jsonOutputFormat() string {
	return `## 出力形式（厳密にこのJSON配列形式で出力）
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
]`
}

func commonRules() string {
	return `## 共通ルール
- 黒板のテキストは正確にOCR
- 数値は単位も含めて正確に
- JSON配列のみ出力。説明文は不要
- remarks は空にせず、必ずマスタの備考から選択
- remarksCandidates はマスタの備考から候補を3つ挙げる
- reasoning は remarks を選んだ根拠を1〜2文で書く`
}

func focusTargetRules() string {
	return `## focusTarget の判定基準
- **全景**: 作業現場全体、重機・車両・作業員が写っている広い構図
- **黒板アップ**: 黒板が画面の大部分を占め、文字が読める状態
- **温度計アップ**: 温度計の表示部分がクローズアップされている
- **その他**: 上記に該当しない場合（材料写真、計器アップ等）`
}

func workTypeHint(workType, variety string) string {
	if workType == "" {
		return "- 工種はマスタの候補から選択（該当なしなら空文字）"
	}
	vHint := ""
	if variety != "" {
		vHint = fmt.Sprintf("\n- 種別は「%s」が基本", variety)
	}
	return fmt.Sprintf("- 工種は基本「%s」%s", workType, vHint)
}

// filterByPhotoType returns rows matching the given photoType.
func filterByPhotoType(rows []HierarchyRow, photoType string) []HierarchyRow {
	var filtered []HierarchyRow
	for _, r := range rows {
		if r.PhotoType == photoType {
			filtered = append(filtered, r)
		}
	}
	return filtered
}

// hierarchyToCompactText renders rows as compact text for prompt injection.
// Format: "写真種別 > 工種 > 種別 > 細別: 備考1, 備考2, ..."
func hierarchyToCompactText(rows []HierarchyRow) string {
	// Group by (photoType, workType, variety, subphase) -> remarks list
	type key struct{ pt, wt, v, sp string }
	order := []key{}
	groups := map[key][]string{}

	for _, r := range rows {
		k := key{r.PhotoType, r.WorkType, r.Variety, r.Subphase}
		if _, exists := groups[k]; !exists {
			order = append(order, k)
		}
		if r.Remarks != "" {
			groups[k] = append(groups[k], r.Remarks)
		}
	}

	var lines []string
	for _, k := range order {
		remarks := strings.Join(groups[k], ", ")
		parts := []string{}
		if k.pt != "" {
			parts = append(parts, k.pt)
		}
		if k.wt != "" {
			parts = append(parts, k.wt)
		}
		if k.v != "" {
			parts = append(parts, k.v)
		}
		if k.sp != "" {
			parts = append(parts, k.sp)
		}
		line := strings.Join(parts, " > ")
		if remarks != "" {
			line += ": " + remarks
		}
		lines = append(lines, line)
	}
	return strings.Join(lines, "\n")
}

// ---- category-specific prompt builders ----

// buildQualityPrompt builds the 品質管理写真 prompt.
// Ported from prompts.rs: build_quality_prompt.
func buildQualityPrompt(images []ImageMeta, rows []HierarchyRow, workType string) string {
	photoList := formatPhotoList(images)
	filtered := filterByPhotoType(rows, "品質管理写真")
	hierarchyText := hierarchyToCompactText(filtered)
	wtHint := workTypeHint(workType, "")
	jsonFmt := jsonOutputFormat()
	common := commonRules()
	focus := focusTargetRules()

	return fmt.Sprintf(
		`あなたは工事写真帳を作成する現場監督です。品質管理写真を解析してください。

## 写真区分
photoCategory は「品質管理写真」固定です。

## マスタ候補
%s

## 制約
%s
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

%s

%s

%s

対象写真:
%s`,
		hierarchyText, wtHint, jsonFmt, focus, common, photoList)
}

// buildConstructionPrompt builds the 施工状況写真 prompt.
// Ported from prompts.rs: build_construction_prompt.
func buildConstructionPrompt(images []ImageMeta, rows []HierarchyRow, workType, variety string) string {
	photoList := formatPhotoList(images)
	filtered := filterByPhotoType(rows, "施工状況写真")
	hierarchyText := hierarchyToCompactText(filtered)
	wtHint := workTypeHint(workType, variety)
	jsonFmt := jsonOutputFormat()
	common := commonRules()
	focus := focusTargetRules()

	return fmt.Sprintf(
		`あなたは工事写真帳を作成する現場監督です。施工状況写真を解析してください。

## 写真区分
photoCategory は「施工状況写真」固定です。

## マスタ候補
%s

## 制約
%s
- 撮影内容（備考）はマスタから選択

## 施工状況写真の判定ポイント
- 乳剤散布状況と養生砂散布状況の判別: スプレイヤーで乳剤を散布する人と飛散防止のベニヤ板を持って立つ人が並ぶ場合は乳剤散布状況
- 処分関連（アスガラ処分）: 黒板に「処分状況」と書かれていれば「アスファルト塊処分状況」、許可票が写っていれば「As塊処分施設許可票」、計量台の上なら「アスファルト塊計量状況」

%s

%s

%s

対象写真:
%s`,
		hierarchyText, wtHint, jsonFmt, focus, common, photoList)
}

// buildSafetyPrompt builds the 安全管理写真 prompt.
// Ported from prompts.rs: build_safety_prompt.
func buildSafetyPrompt(images []ImageMeta, rows []HierarchyRow) string {
	photoList := formatPhotoList(images)
	filtered := filterByPhotoType(rows, "安全管理写真")
	hierarchyText := hierarchyToCompactText(filtered)
	jsonFmt := jsonOutputFormat()
	common := commonRules()
	focus := focusTargetRules()

	return fmt.Sprintf(
		`あなたは工事写真帳を作成する現場監督です。安全管理写真を解析してください。

## 写真区分
photoCategory は「安全管理写真」固定です。

## マスタ候補
%s

## 制約
- 安全管理写真では工種・種別・細別は空
- 撮影内容（備考）はマスタから選択（朝礼、KY活動、新規入場者教育、重機始業前点検、安全パトロール、店社安全パトロール、社外安全パトロール等）
- 黒板の記載内容から適切な備考を判断

%s

%s

%s

対象写真:
%s`,
		hierarchyText, jsonFmt, focus, common, photoList)
}

// buildMeasurementPrompt builds the 出来形管理写真 prompt.
// Ported from prompts.rs: build_measurement_prompt.
func buildMeasurementPrompt(images []ImageMeta, rows []HierarchyRow, workType string) string {
	photoList := formatPhotoList(images)
	filtered := filterByPhotoType(rows, "出来形管理写真")
	hierarchyText := hierarchyToCompactText(filtered)
	wtHint := workTypeHint(workType, "")
	jsonFmt := jsonOutputFormat()
	common := commonRules()
	focus := focusTargetRules()

	return fmt.Sprintf(
		`あなたは工事写真帳を作成する現場監督です。出来形管理写真を解析してください。

## 写真区分
photoCategory は「出来形管理写真」固定です。

## マスタ候補
%s

## 制約
%s
- 撮影内容（備考）はマスタから選択
- 寸法測定値はmeasurementsに記録（例: "50mm", "厚さ45mm"）
- 出来形は3枚1組（全景・管理値・接写）のことが多い

%s

%s

%s

対象写真:
%s`,
		hierarchyText, wtHint, jsonFmt, focus, common, photoList)
}

// buildMaterialPrompt builds the 使用材料写真 prompt.
// Ported from prompts.rs: build_material_prompt.
func buildMaterialPrompt(images []ImageMeta, rows []HierarchyRow, workType string) string {
	photoList := formatPhotoList(images)
	filtered := filterByPhotoType(rows, "使用材料写真")
	hierarchyText := hierarchyToCompactText(filtered)
	wtHint := workTypeHint(workType, "")
	jsonFmt := jsonOutputFormat()
	common := commonRules()
	focus := focusTargetRules()

	return fmt.Sprintf(
		`あなたは工事写真帳を作成する現場監督です。使用材料写真を解析してください。

## 写真区分
photoCategory は「使用材料写真」固定です。

## マスタ候補
%s

## 制約
%s
- 撮影内容（備考）はマスタから選択（材料検収状況、搬入状況等）

%s

%s

%s

対象写真:
%s`,
		hierarchyText, wtHint, jsonFmt, focus, common, photoList)
}

// buildBeforeAfterPrompt builds the 着手前及び完成写真 prompt.
// Ported from prompts.rs: build_before_after_prompt.
func buildBeforeAfterPrompt(images []ImageMeta, rows []HierarchyRow, workType string) string {
	photoList := formatPhotoList(images)
	filtered := filterByPhotoType(rows, "着手前及び完成写真")
	hierarchyText := hierarchyToCompactText(filtered)
	wtHint := workTypeHint(workType, "")
	jsonFmt := jsonOutputFormat()
	common := commonRules()
	focus := focusTargetRules()

	return fmt.Sprintf(
		`あなたは工事写真帳を作成する現場監督です。着手前及び完成写真を解析してください。

## 写真区分
photoCategory は「着手前及び完成写真」固定です。

## マスタ候補
%s

## 制約
%s
- 着手前・完了・竣工のいずれかを判断
- 撮影内容（備考）はマスタから選択

%s

%s

%s

対象写真:
%s`,
		hierarchyText, wtHint, jsonFmt, focus, common, photoList)
}

// buildOtherPrompt builds the その他 prompt.
// Ported from prompts.rs: build_other_prompt.
func buildOtherPrompt(images []ImageMeta, rows []HierarchyRow, workType string) string {
	photoList := formatPhotoList(images)
	filtered := filterByPhotoType(rows, "その他")
	hierarchyText := hierarchyToCompactText(filtered)
	wtHint := workTypeHint(workType, "")
	jsonFmt := jsonOutputFormat()
	common := commonRules()
	focus := focusTargetRules()

	return fmt.Sprintf(
		`あなたは工事写真帳を作成する現場監督です。その他の写真（使用機械等）を解析してください。

## 写真区分
photoCategory は「その他」固定です。

## マスタ候補
%s

## 制約
%s
- 使用機械写真では工種欄が空白でもよい
- 黒板に「使用機械」と書かれている場合、remarks は「使用機械」

%s

%s

%s

対象写真:
%s`,
		hierarchyText, wtHint, jsonFmt, focus, common, photoList)
}

// buildGenericPrompt builds the fallback prompt for unknown photo categories.
// Ported from prompts.rs: build_generic_prompt.
func buildGenericPrompt(images []ImageMeta, rows []HierarchyRow, workType, variety string) string {
	photoList := formatPhotoList(images)
	categories := strings.Join(PhotoCategories, ", ")
	hierarchyText := hierarchyToCompactText(rows)

	var intro string
	if workType != "" {
		intro = fmt.Sprintf("あなたは工事写真帳を作成する現場監督です。工種「%s」の写真を解析してください。", workType)
	} else {
		intro = "あなたは工事写真帳を作成する現場監督です。写真を解析してください。"
	}

	var workTypeConstraint string
	if workType != "" {
		varietyHint := ""
		if variety != "" {
			varietyHint = fmt.Sprintf("\n- 種別は「%s」が基本（確実でない場合は他を選択可）", variety)
		}
		workTypeConstraint = fmt.Sprintf("- 工種は基本「%s」%s", workType, varietyHint)
	} else {
		workTypeConstraint = "- 工種はマスタの候補から選択（該当なしなら空文字）"
	}

	jsonFmt := jsonOutputFormat()
	common := commonRules()
	focus := focusTargetRules()

	return fmt.Sprintf(
		`%s

## 写真区分（写真種別）
以下から最も適切なものを選択：
%s

## 工種マスタ（候補一覧）
各行は「写真種別 > 工種 > 種別 > 細別: 備考1, 備考2, ...」の形式です。
備考（撮影内容）からマスタの候補を1つ選んでください。
%s

## 制約
%s
- ただし、使用機械写真や安全管理写真では工種が空になることがある（異常ではない）
- 撮影内容（備考）だけをマスタから選択（判断不可なら空文字）
- 上位階層はシステム側で自動決定するため、workType/variety/subphase は空文字でよい

## 使用機械写真の判定
- 黒板に「使用機械」と書かれている場合、photoCategory は「その他」、remarks は「使用機械」とする
- 使用機械写真では工種欄が空白でもよい

## 安全管理写真の判定
- 黒板や写真内容から安全管理（朝礼、KY活動、新規入場者教育、重機始業前点検、安全パトロール等）と判断された場合、photoCategory は「安全管理写真」
- 安全管理写真では工種は空

%s

%s

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
- **禁止**: 複数の温度を列挙しない（例: ×「到着160.7℃、敷均し155.4℃」→ ○「155.4℃"）

%s
- 乳剤散布状況と養生砂散布状況の判別: スプレイヤーで乳剤を散布する人と飛散防止のベニヤ板を持って立つ人が並ぶ場合は乳剤散布状況
- 処分関連（アスガラ処分）: 黒板に「処分状況」と書かれていれば「アスファルト塊処分状況」、許可票が写っていれば「As塊処分施設許可票」、計量台の上なら「アスファルト塊計量状況」

対象写真:
%s`,
		intro, categories, hierarchyText, workTypeConstraint,
		jsonFmt, focus, common, photoList)
}
