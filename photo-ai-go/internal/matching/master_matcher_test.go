package matching

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// ---- helpers ----

func mustMasterFromCSV(t *testing.T, csv string) *HierarchyMaster {
	t.Helper()
	m, err := NewHierarchyMasterFromCSVString(csv)
	if err != nil {
		t.Fatalf("NewHierarchyMasterFromCSVString: %v", err)
	}
	return m
}

// ---- port of Rust #[test] blocks ----

// TestDateToMonthDay mirrors test_date_to_month_day.
func TestDateToMonthDay(t *testing.T) {
	cases := []struct {
		input string
		want  string
	}{
		{"2026-02-09 21:23:53", "2月9日"},
		{"2026-12-25 00:00:00", "12月25日"},
		{"", ""},
	}
	for _, c := range cases {
		got := DateToMonthDay(c.input)
		if got != c.want {
			t.Errorf("DateToMonthDay(%q) = %q, want %q", c.input, got, c.want)
		}
	}
}

// TestNormalizeWorkTypeFromOCR mirrors test_normalize_work_type.
func TestNormalizeWorkTypeFromOCR(t *testing.T) {
	cases := []struct{ input, want string }{
		{"舗装補修工", "舗装工"},
		{"舗装工", "舗装工"},
		{"区画線工", "区画線工"},
		{"構造物撤去工", "構造物撤去工"},
		{"", ""},
		{"道路補修工", "道路工"},
	}
	for _, c := range cases {
		got := normalizeWorkTypeFromOCR(c.input)
		if got != c.want {
			t.Errorf("normalizeWorkTypeFromOCR(%q) = %q, want %q", c.input, got, c.want)
		}
	}
}

// TestWorkTypeNormalizationInMatching mirrors test_work_type_normalization_in_matching.
func TestWorkTypeNormalizationInMatching(t *testing.T) {
	csvSrc := `費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,施工状況写真,舗装工,舗装打換え工,表層工,舗設状況,
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,路面切削状況,
`
	master := mustMasterFromCSV(t, csvSrc)
	text := "工種：舗装補修工\n表層工\n舗設状況"

	result := MatchMasterFromDetectedTexts(master, []string{text}, "", "")
	if result == nil {
		t.Fatal("should match despite work_type being 舗装補修工")
	}
	if result.WorkType != "舗装工" {
		t.Errorf("WorkType = %q, want 舗装工", result.WorkType)
	}
	if result.Remarks != "舗設状況" {
		t.Errorf("Remarks = %q, want 舗設状況", result.Remarks)
	}
}

// TestFocusTargetBoostRoadCutting mirrors test_focus_target_boost_road_cutting.
func TestFocusTargetBoostRoadCutting(t *testing.T) {
	csvSrc := `費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,路面切削状況,
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,切削殻積込状況,
`
	master := mustMasterFromCSV(t, csvSrc)
	text := "工事名：テスト工事\n路面切削工\n切削・積込状況"
	texts := []string{text}

	// No focus_target → 切削殻積込状況 wins (higher bigram overlap).
	r := MatchMasterFromDetectedTexts(master, texts, "", "")
	if r == nil {
		t.Fatal("expected match (no focus_target)")
	}
	if r.Remarks != "切削殻積込状況" {
		t.Errorf("no-ft: Remarks = %q, want 切削殻積込状況", r.Remarks)
	}

	// With focus_target → 路面切削状況 wins.
	r2 := MatchMasterFromDetectedTexts(master, texts, "", "路面切削状況")
	if r2 == nil {
		t.Fatal("expected match (with focus_target)")
	}
	if r2.Remarks != "路面切削状況" {
		t.Errorf("ft=路面切削状況: Remarks = %q, want 路面切削状況", r2.Remarks)
	}
}

// TestPhase1TiebreakWithFocusTarget mirrors test_phase1_tiebreak_with_focus_target.
func TestPhase1TiebreakWithFocusTarget(t *testing.T) {
	csvSrc := `費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,品質管理写真,舗装工,舗装打換え工,表層工,到着温度,到着温度|出荷温度|合材到着
直接工事費,品質管理写真,舗装工,舗装打換え工,表層工,敷均し温度,敷均し温度|敷きならし|舗設温度
直接工事費,品質管理写真,舗装工,舗装打換え工,表層工,初期締固め前温度,初期締固|初転圧前|初期転圧温度|初期転圧前
`
	master := mustMasterFromCSV(t, csvSrc)
	text := "表層工\n到着温度 159.7\n敷均し温度 154.8\n初期転圧前温度 148.1"
	texts := []string{text}

	cases := []struct{ ft, want string }{
		{"敷均し温度", "敷均し温度"},
		{"初期転圧前温度", "初期締固め前温度"},
		{"到着温度", "到着温度"},
	}
	for _, c := range cases {
		r := MatchMasterFromDetectedTexts(master, texts, "", c.ft)
		if r == nil {
			t.Fatalf("ft=%q: no match", c.ft)
		}
		if r.Remarks != c.want {
			t.Errorf("ft=%q: Remarks = %q, want %q", c.ft, r.Remarks, c.want)
		}
	}
}

// TestDekigataFolderCuttingMatchesRoadCutting mirrors
// test_dekigata_folder_切削出来形_matches_路面切削工.
func TestDekigataFolderCuttingMatchesRoadCutting(t *testing.T) {
	csvSrc := `費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,出来形管理写真,舗装工,舗装打換え工,上層路盤工,不陸整正出来形,路盤出来形|出来形検測|路盤|基準高下がり|基準高
直接工事費,出来形管理写真,舗装工,舗装打換え工,上層路盤工,不陸整正出来形・管理値,
直接工事費,出来形管理写真,舗装工,路面切削工,路面切削,路面切削工出来形測定,切削出来形|切削工出来形
`
	master := mustMasterFromCSV(t, csvSrc)
	text := "出来形管理用紙 No.9"

	r := MatchMasterFromDetectedTexts(master, []string{text}, "切削出来形", "")
	if r == nil {
		t.Fatal("expected match")
	}
	if r.Remarks != "路面切削工出来形測定" {
		t.Errorf("Remarks = %q, want 路面切削工出来形測定", r.Remarks)
	}
}

// TestDekigataFolderFurikuseiriStillMatches mirrors
// test_dekigata_folder_不陸整正_still_matches.
func TestDekigataFolderFurikuseiriStillMatches(t *testing.T) {
	csvSrc := `費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,出来形管理写真,舗装工,舗装打換え工,上層路盤工,不陸整正出来形,路盤出来形|出来形検測|路盤|基準高下がり|基準高
直接工事費,出来形管理写真,舗装工,舗装打換え工,上層路盤工,不陸整正出来形・管理値,
直接工事費,出来形管理写真,舗装工,路面切削工,路面切削,路面切削工出来形測定,切削出来形|切削工出来形
`
	master := mustMasterFromCSV(t, csvSrc)
	text := "出来形管理用紙 No.3\n基準高"

	r := MatchMasterFromDetectedTexts(master, []string{text}, "不陸整正", "")
	if r == nil {
		t.Fatal("expected match")
	}
	if r.Remarks != "不陸整正出来形" {
		t.Errorf("Remarks = %q, want 不陸整正出来形", r.Remarks)
	}
}

// TestExtractVarietyFromFolder mirrors test_extract_variety_from_folder.
func TestExtractVarietyFromFolder(t *testing.T) {
	csvSrc := `費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,施工状況写真,舗装工,舗装打換え工,表層工,舗設状況,
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,路面切削状況,
直接工事費,施工状況写真,舗装工,切削オーバーレイ工,表層工,舗設状況,
`
	master := mustMasterFromCSV(t, csvSrc)

	cases := []struct {
		folder string
		want   string // empty = "no match"
	}{
		{"舗装打換え", "舗装打換え工"},
		{"路面切削出来形", "路面切削工"},
		{"温度管理", ""},
	}
	for _, c := range cases {
		got := extractVarietyFromFolder(master, c.folder)
		if got != c.want {
			t.Errorf("extractVarietyFromFolder(%q) = %q, want %q", c.folder, got, c.want)
		}
	}
}

// TestExtractKnownFolderTermsNoisyName mirrors
// test_extract_known_folder_terms_from_noisy_folder_name.
func TestExtractKnownFolderTermsNoisyName(t *testing.T) {
	csvSrc := `費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,路面切削状況,切削状況|施工状況
`
	master := mustMasterFromCSV(t, csvSrc)
	terms := extractKnownFolderTerms(master, "疎通_施工状況_1枚")

	found := false
	for _, t2 := range terms {
		if t2 == "施工状況" {
			found = true
		}
	}
	if !found {
		t.Errorf("expected '施工状況' in terms, got %v", terms)
	}
}

// TestFolderHasMasterEntryNoisyName mirrors
// test_folder_has_master_entry_with_noisy_folder_name.
func TestFolderHasMasterEntryNoisyName(t *testing.T) {
	csvSrc := `費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,路面切削状況,切削状況|施工状況
`
	master := mustMasterFromCSV(t, csvSrc)
	if !FolderHasMasterEntry(master, "疎通_施工状況_1枚") {
		t.Error("FolderHasMasterEntry should return true for noisy folder containing known term")
	}
}

// TestFolderKeywordMatchesPhotoType mirrors
// test_folder_keyword_can_match_photo_type_without_exact_folder_match.
func TestFolderKeywordMatchesPhotoType(t *testing.T) {
	csvSrc := `費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,路面切削状況,
直接工事費,品質管理写真,舗装工,舗装打換え工,表層工,到着温度,
`
	master := mustMasterFromCSV(t, csvSrc)
	text := "工種：舗装工"

	r := MatchMasterFromDetectedTexts(master, []string{text}, "疎通_施工状況_1枚", "")
	if r == nil {
		t.Fatal("expected match")
	}
	if r.PhotoType != "施工状況写真" {
		t.Errorf("PhotoType = %q, want 施工状況写真", r.PhotoType)
	}
}

// TestFocusTargetNoEffectWhenEmpty mirrors test_focus_target_no_effect_when_empty.
func TestFocusTargetNoEffectWhenEmpty(t *testing.T) {
	csvSrc := `費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,施工状況写真,舗装工,舗装打換え工,表層工,舗設状況,
直接工事費,施工状況写真,舗装工,舗装打換え工,表層工,初期転圧状況,
`
	master := mustMasterFromCSV(t, csvSrc)
	text := "表層工\n舗設状況"

	rNone := MatchMasterFromDetectedTexts(master, []string{text}, "", "")
	rEmpty := MatchMasterFromDetectedTexts(master, []string{text}, "", "")
	if rNone == nil || rEmpty == nil {
		t.Fatal("expected matches")
	}
	if rNone.Remarks != rEmpty.Remarks {
		t.Errorf("empty ft should behave identically: %q vs %q", rNone.Remarks, rEmpty.Remarks)
	}
	if rNone.Remarks != "舗設状況" {
		t.Errorf("Remarks = %q, want 舗設状況", rNone.Remarks)
	}
}

// TestConstructionStatusFolderExcludesOther mirrors
// test_construction_status_folder_excludes_other_candidates.
func TestConstructionStatusFolderExcludesOther(t *testing.T) {
	csvSrc := `費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,その他,舗装工,,,コンバインドローラー,
直接工事費,施工状況写真,舗装工,切削オーバーレイ工,表層工,初期転圧状況,初期転圧|転圧状況
`
	master := mustMasterFromCSV(t, csvSrc)
	texts := []string{"表層工\n初期転圧状況\nコンバインドローラー"}

	r := MatchMasterFromDetectedTexts(master, texts, "施工状況", "初期転圧状況")
	if r == nil {
		t.Fatal("expected match")
	}
	if r.PhotoType != "施工状況写真" {
		t.Errorf("PhotoType = %q, want 施工状況写真", r.PhotoType)
	}
	if r.Remarks != "初期転圧状況" {
		t.Errorf("Remarks = %q, want 初期転圧状況", r.Remarks)
	}
}

// TestFocusTargetMatchesSearchPatternDirectly mirrors
// test_focus_target_matches_search_pattern_directly.
func TestFocusTargetMatchesSearchPatternDirectly(t *testing.T) {
	csvSrc := `費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,出来形管理写真,舗装工,切削オーバーレイ工,表層工,コアー採取前,コアー採取前|コア採取前
直接工事費,出来形管理写真,舗装工,切削オーバーレイ工,表層工,コアー採取状況,コアー採取状況|コア採取状況|コアー採集
直接工事費,出来形管理写真,舗装工,切削オーバーレイ工,表層工,コアー厚さ測定,コアー厚さ測定|コア厚測定|舗装厚測定|コア厚さ
直接工事費,出来形管理写真,舗装工,切削オーバーレイ工,表層工,コアー復築前,コアー復築前|コア復築前
直接工事費,出来形管理写真,舗装工,切削オーバーレイ工,表層工,コアー復築状況,コアー復築状況|コア復築状況|コアー復築
直接工事費,出来形管理写真,舗装工,切削オーバーレイ工,表層工,コアー復築完了,コアー復築完了|コア復築完了
`
	master := mustMasterFromCSV(t, csvSrc)

	cases := []struct {
		text     string
		folder   string
		ft       string
		wantRmk  string
	}{
		{
			"サンプル道路舗装工事, 舗装工 表層工, No.8 R",
			"コアNo.8",
			"コアー厚さ測定",
			"コアー厚さ測定",
		},
		{
			"55mm",
			"コアNo.8",
			"舗装厚測定・接写",
			"コアー厚さ測定",
		},
		{
			"工事名：テスト工事, 場所：No.1 R, 表層工 コアー 採取前",
			"コアNo.1",
			"コアー採取前",
			"コアー採取前",
		},
		{
			"工事名 テスト工事, 場所 No. 1R, 表層工 コアー 復築前",
			"コアNo.1",
			"コアー復築前",
			"コアー復築前",
		},
	}
	for _, c := range cases {
		r := MatchMasterFromDetectedTexts(master, []string{c.text}, c.folder, c.ft)
		if r == nil {
			t.Fatalf("ft=%q: no match", c.ft)
		}
		if r.Remarks != c.wantRmk {
			t.Errorf("ft=%q: Remarks = %q, want %q", c.ft, r.Remarks, c.wantRmk)
		}
	}
}

// TestEmptyWorkTypeOCRDoesNotFilterAllRows mirrors
// test_empty_work_type_in_ocr_does_not_filter_out_all_rows.
func TestEmptyWorkTypeOCRDoesNotFilterAllRows(t *testing.T) {
	csvSrc := `費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,切削完了,切削完了
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,端部舗装版撤去状況,端部舗装版撤去
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,不純物の除去確認,不純物.*除去|除去確認
`
	master := mustMasterFromCSV(t, csvSrc)

	text1 := "工事名:テスト工事, 工種:, 測点:No.12, 切削完了"
	r1 := MatchMasterFromDetectedTexts(master, []string{text1}, "施工状況", "切削完了")
	if r1 == nil {
		t.Fatal("should match 切削完了 despite empty 工種:")
	}
	if r1.Remarks != "切削完了" {
		t.Errorf("Remarks = %q, want 切削完了", r1.Remarks)
	}

	text2 := "工事名:テスト工事, 工種:, 測点:No.12, 端部舗装版撤去状況"
	r2 := MatchMasterFromDetectedTexts(master, []string{text2}, "施工状況", "端部舗装版撤去状況")
	if r2 == nil {
		t.Fatal("should match 端部舗装版撤去状況")
	}
	if r2.Remarks != "端部舗装版撤去状況" {
		t.Errorf("Remarks = %q, want 端部舗装版撤去状況", r2.Remarks)
	}
}

// TestKoushuValueThatIsActuallyVarietyDoesNotFilterWorkType mirrors
// test_koushu_value_that_is_actually_variety_does_not_filter_out_work_type.
func TestKoushuValueThatIsActuallyVarietyDoesNotFilterWorkType(t *testing.T) {
	csvSrc := `費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,路面切削状況,
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,切削殻積込状況,
`
	master := mustMasterFromCSV(t, csvSrc)
	text := "場所: 取付 No.1 L\n工種: 路面切削工\n状況: 切削・積込状況"

	r := MatchMasterFromDetectedTexts(master, []string{text}, "疎通_施工状況_1枚", "作業状況")
	if r == nil {
		t.Fatal("variety-like 工種 should not filter out 舗装工 rows")
	}
	if r.PhotoType != "施工状況写真" {
		t.Errorf("PhotoType = %q, want 施工状況写真", r.PhotoType)
	}
}

// ---- CSV parsing tests ----

// TestParseHierarchyCSVBasic verifies basic parsing.
func TestParseHierarchyCSVBasic(t *testing.T) {
	csvSrc := "費目,写真区分,工種,種別,細別,備考,検索パターン\n" +
		"直接工事費,施工状況写真,舗装工,舗装打換え工,表層工,舗設状況,\n"
	rows, err := parseHierarchyCSV(strings.NewReader(csvSrc))
	if err != nil {
		t.Fatalf("parseHierarchyCSV: %v", err)
	}
	if len(rows) != 1 {
		t.Fatalf("expected 1 row, got %d", len(rows))
	}
	r := rows[0]
	if r.PhotoDivision != "直接工事費" {
		t.Errorf("PhotoDivision = %q", r.PhotoDivision)
	}
	if r.WorkType != "舗装工" {
		t.Errorf("WorkType = %q", r.WorkType)
	}
	if r.Remarks != "舗設状況" {
		t.Errorf("Remarks = %q", r.Remarks)
	}
}

// TestParseHierarchyCSVAlternativeHeader verifies "撮影内容" alias.
func TestParseHierarchyCSVAlternativeHeader(t *testing.T) {
	csvSrc := "費目,写真区分,工種,種別,細別,撮影内容,検索パターン\n" +
		"直接工事費,施工状況写真,舗装工,舗装打換え工,表層工,舗設状況,\n"
	rows, err := parseHierarchyCSV(strings.NewReader(csvSrc))
	if err != nil {
		t.Fatalf("parseHierarchyCSV: %v", err)
	}
	if rows[0].Remarks != "舗設状況" {
		t.Errorf("Remarks = %q (撮影内容 alias failed)", rows[0].Remarks)
	}
}

// TestParseHierarchyCSVQuoted verifies quoted fields.
func TestParseHierarchyCSVQuoted(t *testing.T) {
	csvSrc := "費目,写真区分,工種,種別,細別,備考,検索パターン\n" +
		"\"直接工事費\",\"施工状況写真\",\"舗装工\",\"舗装打換え工\",\"表層工\",\"舗設状況\",\"\"\n"
	rows, err := parseHierarchyCSV(strings.NewReader(csvSrc))
	if err != nil {
		t.Fatalf("parseHierarchyCSV: %v", err)
	}
	if rows[0].PhotoDivision != "直接工事費" {
		t.Errorf("PhotoDivision = %q", rows[0].PhotoDivision)
	}
}

// TestParseHierarchyCSVSearchPatterns verifies multi-pattern splitting.
func TestParseHierarchyCSVSearchPatterns(t *testing.T) {
	csvSrc := "費目,写真区分,工種,種別,細別,備考,検索パターン\n" +
		"直接工事費,品質管理写真,舗装工,舗装打換え工,表層工,温度測定,温度管理|到着温度\n"
	rows, err := parseHierarchyCSV(strings.NewReader(csvSrc))
	if err != nil {
		t.Fatalf("parseHierarchyCSV: %v", err)
	}
	if rows[0].SearchPatterns != "温度管理|到着温度" {
		t.Errorf("SearchPatterns = %q", rows[0].SearchPatterns)
	}
}

// ---- golden test: real CSV from master/by_work_type/ ----

// TestGoldenRealCSV loads 仮設工.csv (PII-free: no 株式会社/有限会社/建設/組),
// feeds synthetic photo-metadata fixtures, and asserts expected matches.
// PII check: rg -E '(株式会社|有限会社|建設|組)' master/by_work_type/仮設工.csv → 0 hits.
func TestGoldenRealCSV(t *testing.T) {
	// Locate repo root relative to this test file.
	// Walk up from the test's directory until we find master/.
	dir, err := findRepoRoot()
	if err != nil {
		t.Skipf("cannot locate repo root (not a concern in CI with fixed path): %v", err)
	}
	csvPath := filepath.Join(dir, "master", "by_work_type", "仮設工.csv")
	if _, err := os.Stat(csvPath); os.IsNotExist(err) {
		t.Skipf("CSV not found at %s, skipping golden test", csvPath)
	}

	master, err := NewHierarchyMasterFromCSVFile(csvPath)
	if err != nil {
		t.Fatalf("NewHierarchyMasterFromCSVFile: %v", err)
	}
	if len(master.rows) == 0 {
		t.Fatal("master has zero rows — CSV load failed silently")
	}

	type fixture struct {
		desc        string
		texts       []string
		folder      string
		ft          string
		wantRemarks string
		wantPhotoType string
	}
	// PII-free fixtures using dummy names as required by repo rules.
	fixtures := []fixture{
		{
			desc:        "誘導員配置状況 should match by search in remarks",
			texts:       []string{"工種：仮設工\n交通管理工\n誘導員配置状況"},
			folder:      "施工状況",
			ft:          "誘導員配置状況",
			wantRemarks: "誘導員配置状況",
			wantPhotoType: "施工状況写真",
		},
		{
			desc:        "保安施設設置状況 should match",
			texts:       []string{"工種：仮設工\n保安施設設置状況"},
			folder:      "",
			ft:          "保安施設設置状況",
			wantRemarks: "保安施設設置状況",
			wantPhotoType: "施工状況写真",
		},
		{
			desc:        "完了 should match via remarks exact hit",
			texts:       []string{"仮設工 交通管理工\n完了"},
			folder:      "",
			ft:          "完了",
			wantRemarks: "完了",
			wantPhotoType: "施工状況写真",
		},
	}

	for _, f := range fixtures {
		t.Run(f.desc, func(t *testing.T) {
			r := MatchMasterFromDetectedTexts(master, f.texts, f.folder, f.ft)
			if r == nil {
				t.Fatalf("no match returned")
			}
			if r.Remarks != f.wantRemarks {
				t.Errorf("Remarks = %q, want %q", r.Remarks, f.wantRemarks)
			}
			if r.PhotoType != f.wantPhotoType {
				t.Errorf("PhotoType = %q, want %q", r.PhotoType, f.wantPhotoType)
			}
		})
	}
}

// findRepoRoot walks up from the package directory to find the repo root
// (identified by the presence of "master/by_work_type" directory).
func findRepoRoot() (string, error) {
	// Start from current working directory.
	dir, err := os.Getwd()
	if err != nil {
		return "", err
	}
	for {
		candidate := filepath.Join(dir, "master", "by_work_type")
		if info, err := os.Stat(candidate); err == nil && info.IsDir() {
			return dir, nil
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			break
		}
		dir = parent
	}
	return "", os.ErrNotExist
}
