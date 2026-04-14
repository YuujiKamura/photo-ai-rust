// Package matching — master_matcher.go
//
// Ports master_matcher.rs from the Rust crate to Go.
// The algorithm is a two-phase scorer:
//
//	Phase 1: search_pattern (|–delimited) keyword hit  → priority score 1000
//	Phase 2: remarks/photo_type token overlap + focus_target boost
//
// All bigram utilities are reused from matching.go in this package.
package matching

import (
	"encoding/csv"
	"fmt"
	"io"
	"os"
	"sort"
	"strings"
	"unicode/utf8"
)

// ---- scoring constants (mirrors Rust) ----

const (
	exactMatchScore              = 100
	partialMatchScore            = 50
	focusTargetBoostMultiplier   = 3
	searchPatternPriorityScore   = 1000
)

// HierarchyRow mirrors Rust's HierarchyRow / CsvRow.
// CSV columns: 費目, 写真区分, 工種, 種別, 細別, 備考(or 撮影内容), 検索パターン
type HierarchyRow struct {
	PhotoDivision  string // 費目
	PhotoType      string // 写真区分
	WorkType       string // 工種
	Variety        string // 種別
	Subphase       string // 細別
	Remarks        string // 備考 / 撮影内容
	SearchPatterns string // 検索パターン (|–delimited)
}

// HierarchyMaster holds all rows from one or more master CSVs.
type HierarchyMaster struct {
	rows []HierarchyRow
}

// Rows returns all loaded rows.
func (m *HierarchyMaster) Rows() []HierarchyRow { return m.rows }

// NewHierarchyMasterFromRows builds a HierarchyMaster directly from rows (for tests).
func NewHierarchyMasterFromRows(rows []HierarchyRow) *HierarchyMaster {
	return &HierarchyMaster{rows: rows}
}

// NewHierarchyMasterFromCSVString parses a CSV string into a HierarchyMaster.
// Accepts both "備考" and "撮影内容" as the remarks column header.
func NewHierarchyMasterFromCSVString(content string) (*HierarchyMaster, error) {
	rows, err := parseHierarchyCSV(strings.NewReader(content))
	if err != nil {
		return nil, err
	}
	return &HierarchyMaster{rows: rows}, nil
}

// NewHierarchyMasterFromCSVFile loads a CSV file into a HierarchyMaster.
func NewHierarchyMasterFromCSVFile(path string) (*HierarchyMaster, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	rows, err := parseHierarchyCSV(f)
	if err != nil {
		return nil, err
	}
	return &HierarchyMaster{rows: rows}, nil
}

// parseHierarchyCSV is the internal CSV reader (encoding/csv, no new deps).
func parseHierarchyCSV(r io.Reader) ([]HierarchyRow, error) {
	cr := csv.NewReader(r)
	cr.LazyQuotes = true
	cr.TrimLeadingSpace = true

	header, err := cr.Read()
	if err != nil {
		return nil, fmt.Errorf("csv header: %w", err)
	}

	// Map column names to indices.
	idx := map[string]int{}
	for i, h := range header {
		idx[strings.TrimSpace(h)] = i
	}

	mustCol := func(names ...string) (int, error) {
		for _, n := range names {
			if i, ok := idx[n]; ok {
				return i, nil
			}
		}
		return -1, fmt.Errorf("csv column not found: %v", names)
	}

	colPhotoDivision, err := mustCol("費目")
	if err != nil {
		return nil, err
	}
	colPhotoType, err := mustCol("写真区分")
	if err != nil {
		return nil, err
	}
	colWorkType, err := mustCol("工種")
	if err != nil {
		return nil, err
	}
	colVariety, err := mustCol("種別")
	if err != nil {
		return nil, err
	}
	colSubphase, err := mustCol("細別")
	if err != nil {
		return nil, err
	}
	colRemarks, err := mustCol("備考", "撮影内容")
	if err != nil {
		return nil, err
	}
	colSearchPatterns, err := mustCol("検索パターン")
	if err != nil {
		return nil, err
	}

	safeGet := func(rec []string, col int) string {
		if col < 0 || col >= len(rec) {
			return ""
		}
		return strings.TrimSpace(rec[col])
	}

	var rows []HierarchyRow
	lineNum := 1
	for {
		rec, err := cr.Read()
		if err == io.EOF {
			break
		}
		lineNum++
		if err != nil {
			return nil, fmt.Errorf("csv line %d: %w", lineNum, err)
		}
		row := HierarchyRow{
			PhotoDivision:  safeGet(rec, colPhotoDivision),
			PhotoType:      safeGet(rec, colPhotoType),
			WorkType:       safeGet(rec, colWorkType),
			Variety:        safeGet(rec, colVariety),
			Subphase:       safeGet(rec, colSubphase),
			Remarks:        strings.ReplaceAll(safeGet(rec, colRemarks), `\n`, "\n"),
			SearchPatterns: safeGet(rec, colSearchPatterns),
		}
		rows = append(rows, row)
	}
	return rows, nil
}

// ---- token overlap scorer (mirrors Rust token_overlap_score) ----

// tokenOverlapScore returns an integer overlap score between two strings using
// character bigrams, with short-circuit boosts for exact/substring matches.
func tokenOverlapScore(a, b string) int {
	if a == "" || b == "" {
		return 0
	}
	if a == b {
		return exactMatchScore
	}
	if strings.Contains(b, a) || strings.Contains(a, b) {
		return partialMatchScore
	}
	aRunes := []rune(a)
	bRunes := []rune(b)
	if len(aRunes) < 2 || len(bRunes) < 2 {
		return 0
	}
	// Set-based bigram intersection (mirrors Rust HashSet<(char,char)>).
	aBG := make(map[string]struct{}, len(aRunes)-1)
	for i := 0; i < len(aRunes)-1; i++ {
		aBG[bigramKey(aRunes[i], aRunes[i+1])] = struct{}{}
	}
	bBG := make(map[string]struct{}, len(bRunes)-1)
	for i := 0; i < len(bRunes)-1; i++ {
		bBG[bigramKey(bRunes[i], bRunes[i+1])] = struct{}{}
	}
	count := 0
	for k := range aBG {
		if _, ok := bBG[k]; ok {
			count++
		}
	}
	return count
}

// ---- folder-name helpers (mirrors Rust) ----

// expandFolderTermVariants generates lookup variants for a master term.
// Strips trailing "写真" to allow "施工状況" to match "施工状況写真" columns.
func expandFolderTermVariants(term string) []string {
	term = strings.TrimSpace(term)
	if term == "" {
		return nil
	}
	variants := []string{term}
	if stripped := strings.TrimSuffix(term, "写真"); stripped != term {
		stripped = strings.TrimSpace(stripped)
		if utf8.RuneCountInString(stripped) >= 2 {
			variants = append(variants, stripped)
		}
	}
	// Sort longest first, dedup.
	sort.Slice(variants, func(i, j int) bool {
		return utf8.RuneCountInString(variants[i]) > utf8.RuneCountInString(variants[j])
	})
	seen := map[string]bool{}
	out := variants[:0]
	for _, v := range variants {
		if !seen[v] {
			seen[v] = true
			out = append(out, v)
		}
	}
	return out
}

// extractKnownFolderTerms mines master-known terms from folderName.
// Greedy: longest match consumed first (mirrors Rust).
func extractKnownFolderTerms(master *HierarchyMaster, folderName string) []string {
	if folderName == "" {
		return nil
	}

	// Build all candidate terms from master rows (long first).
	var terms []string
	seen := map[string]bool{}
	addTerm := func(raw string) {
		for _, v := range expandFolderTermVariants(raw) {
			if utf8.RuneCountInString(v) >= 2 && !seen[v] {
				seen[v] = true
				terms = append(terms, v)
			}
		}
	}

	for _, row := range master.rows {
		addTerm(row.PhotoType)
		addTerm(row.Remarks)
		addTerm(row.Variety)
		addTerm(row.Subphase)
		for _, pat := range strings.Split(row.SearchPatterns, "|") {
			addTerm(strings.TrimSpace(pat))
		}
	}

	sort.Slice(terms, func(i, j int) bool {
		return utf8.RuneCountInString(terms[i]) > utf8.RuneCountInString(terms[j])
	})

	remaining := folderName
	var hits []string
	for _, term := range terms {
		if strings.Contains(remaining, term) {
			hits = append(hits, term)
			remaining = strings.ReplaceAll(remaining, term, " ")
		}
	}
	return hits
}

// extractVarietyFromFolder infers variety from folderName by bidirectional
// substring matching against master varieties (minus trailing "工").
func extractVarietyFromFolder(master *HierarchyMaster, folderName string) string {
	// Collect unique varieties.
	varietySet := map[string]bool{}
	for _, row := range master.rows {
		if row.Variety != "" {
			varietySet[row.Variety] = true
		}
	}

	bestVariety := ""
	bestLen := 0
	for v := range varietySet {
		core := v
		if strings.HasSuffix(v, "工") {
			core = v[:len(v)-len("工")]
		}
		coreLen := utf8.RuneCountInString(core)
		overlap := 0
		if strings.Contains(folderName, core) {
			overlap = coreLen
		} else if strings.Contains(core, folderName) {
			overlap = utf8.RuneCountInString(folderName)
		}
		if overlap >= 2 && overlap > bestLen {
			bestLen = overlap
			bestVariety = v
		}
	}
	return bestVariety
}

// shouldExcludeOtherForFolder returns true when the folder name strongly signals
// a 施工状況 context (exclude photo_type="その他" rows).
func shouldExcludeOtherForFolder(master *HierarchyMaster, folderName string) bool {
	if folderName == "" {
		return false
	}
	for _, term := range extractKnownFolderTerms(master, folderName) {
		if term == "施工状況" || term == "施工状況写真" {
			return true
		}
	}
	return false
}

// ---- keyword extraction (mirrors Rust extract_match_keywords) ----

// extractMatchKeywords parses detected texts and folder name into
// (keywords, workTypeHint, varietyHint).
//
// Keys recognised: 工種, 工事名, 車番, 車両番号, 場所, 測点 — rest go to keywords.
func extractMatchKeywords(
	master *HierarchyMaster,
	detectedTexts []string,
	folderName string,
) (keywords []string, workTypeHint string, varietyHint string) {
	// hasWorkType checks master rows for an exact work_type match.
	hasWorkType := func(s string) bool {
		for _, row := range master.rows {
			if row.WorkType == s {
				return true
			}
		}
		return false
	}
	hasVariety := func(s string) bool {
		for _, row := range master.rows {
			if row.Variety == s || row.Subphase == s {
				return true
			}
		}
		return false
	}

	for _, text := range detectedTexts {
		for k, v := range extractKVFromText(text) {
			switch k {
			case "工種":
				if v == "" {
					break
				}
				norm := normalizeWorkTypeFromOCR(v)
				switch {
				case hasWorkType(norm):
					workTypeHint = norm
				case hasVariety(v):
					if varietyHint == "" {
						varietyHint = v
					}
				default:
					keywords = append(keywords, v)
				}
			case "工事名", "車番", "車両番号", "場所", "測点":
				// skip — not used for matching
			case "":
				if v != "" {
					keywords = append(keywords, v)
				}
			default:
				keywords = append(keywords, k)
				if v != "" {
					keywords = append(keywords, v)
				}
			}
		}
	}

	// Promote keyword to varietyHint if it exactly matches a master variety/subphase.
	if varietyHint == "" {
		for _, kw := range keywords {
			if hasVariety(kw) {
				varietyHint = kw
				break
			}
		}
	}
	// Remove varietyHint from keyword list (reduce noise).
	if varietyHint != "" {
		filtered := keywords[:0]
		for _, kw := range keywords {
			if kw != varietyHint {
				filtered = append(filtered, kw)
			}
		}
		keywords = filtered
	}

	// Append master-known terms from folder name.
	keywords = append(keywords, extractKnownFolderTerms(master, folderName)...)

	// Fallback: infer varietyHint from folder name if still unset.
	if varietyHint == "" && folderName != "" {
		if v := extractVarietyFromFolder(master, folderName); v != "" {
			varietyHint = v
		}
	}

	return keywords, workTypeHint, varietyHint
}

// extractKVFromText is a lightweight port of Rust's extract_kv_from_text.
// Splits text into lines, then each line on "：" or ":" into key/value.
// Lines without a separator are returned with key="".
func extractKVFromText(text string) map[string]string {
	result := map[string]string{}
	for _, line := range strings.Split(text, "\n") {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		// Try full-width colon first, then half-width.
		sep := "："
		if !strings.Contains(line, sep) {
			sep = ":"
		}
		parts := strings.SplitN(line, sep, 2)
		if len(parts) == 2 {
			k := strings.TrimSpace(parts[0])
			v := strings.TrimSpace(parts[1])
			result[k] = v
		} else {
			// Keyless: accumulate into "" key (last value wins per line).
			result[""] = line
		}
	}
	return result
}

// normalizeWorkTypeFromOCR mirrors Rust's normalize_work_type_from_ocr:
// strips common synonyms/suffixes so "舗装補修工" → "舗装工".
func normalizeWorkTypeFromOCR(s string) string {
	// Direct replacements used in Rust policy module.
	replacements := []struct{ from, to string }{
		{"舗装補修工", "舗装工"},
		{"道路補修工", "道路工"},
	}
	for _, r := range replacements {
		if s == r.from {
			return r.to
		}
	}
	return s
}

// ---- candidate scoring (mirrors Rust score_candidates) ----

type scoredRow struct {
	row   HierarchyRow
	score int
}

// scoreCandidates implements the two-phase scoring.
func scoreCandidates(
	master *HierarchyMaster,
	keywords []string,
	workTypeHint string,
	varietyHint string,
	focusTarget string,
	excludeOther bool,
) []scoredRow {
	// Filter master rows.
	var candidates []HierarchyRow
	for _, row := range master.rows {
		if excludeOther && row.PhotoType == "その他" {
			continue
		}
		if workTypeHint != "" && row.WorkType != workTypeHint && row.WorkType != "" {
			continue
		}
		if varietyHint != "" {
			if row.Variety != varietyHint &&
				row.Subphase != varietyHint &&
				!strings.Contains(row.Variety, varietyHint) &&
				!strings.Contains(varietyHint, row.Variety) {
				continue
			}
		}
		candidates = append(candidates, row)
	}

	ft := focusTarget

	// ---- Phase 1: search_pattern keyword hit ----
	type phase1Hit struct {
		row   HierarchyRow
		score int
	}
	var phase1Hits []phase1Hit

	for _, row := range candidates {
		if row.SearchPatterns == "" {
			continue
		}
		patterns := splitPatterns(row.SearchPatterns)
		score := 0
		for _, kw := range keywords {
			for _, p := range patterns {
				if strings.Contains(kw, p) || strings.Contains(p, kw) {
					score++
					break
				}
			}
		}
		// focus_target also checked against search patterns.
		if ft != "" {
			for _, p := range patterns {
				if strings.Contains(ft, p) || strings.Contains(p, ft) {
					score++
					break
				}
			}
		}
		if score > 0 {
			phase1Hits = append(phase1Hits, phase1Hit{row: row, score: score})
		}
	}

	if len(phase1Hits) > 0 {
		maxScore := 0
		for _, h := range phase1Hits {
			if h.score > maxScore {
				maxScore = h.score
			}
		}
		var best []phase1Hit
		for _, h := range phase1Hits {
			if h.score == maxScore {
				best = append(best, h)
			}
		}

		if len(best) == 1 {
			return []scoredRow{{row: best[0].row, score: searchPatternPriorityScore}}
		}

		// Tie-break: focus_target vs remarks token overlap.
		if ft != "" {
			winner := best[0]
			winnerScore := tokenOverlapScore(ft, best[0].row.Remarks)
			for _, h := range best[1:] {
				s := tokenOverlapScore(ft, h.row.Remarks)
				if s > winnerScore {
					winnerScore = s
					winner = h
				}
			}
			return []scoredRow{{row: winner.row, score: searchPatternPriorityScore}}
		}
		// No focus_target → first best.
		return []scoredRow{{row: best[0].row, score: searchPatternPriorityScore}}
	}

	// ---- Phase 2: remarks / photo_type token overlap + focus_target boost ----
	var scored []scoredRow
	for _, row := range candidates {
		if row.Remarks == "" && row.PhotoType == "" {
			continue
		}
		kwScore := 0
		for _, kw := range keywords {
			rs := tokenOverlapScore(kw, row.Remarks)
			ps := tokenOverlapScore(kw, row.PhotoType)
			if rs > ps {
				kwScore += rs
			} else {
				kwScore += ps
			}
		}
		ftBoost := 0
		if ft != "" {
			ftBoost = tokenOverlapScore(ft, row.Remarks) * focusTargetBoostMultiplier
		}
		total := kwScore + ftBoost
		if total > 0 {
			scored = append(scored, scoredRow{row: row, score: total})
		}
	}
	return scored
}

// splitPatterns splits a "|"-delimited search_patterns string.
func splitPatterns(s string) []string {
	parts := strings.Split(s, "|")
	out := parts[:0]
	for _, p := range parts {
		p = strings.TrimSpace(p)
		if p != "" {
			out = append(out, p)
		}
	}
	return out
}

// ---- public API ----

// MatchMasterFromDetectedTexts is the main entry point, mirroring Rust's
// match_master_from_detected_texts. Returns nil if no candidate was found.
func MatchMasterFromDetectedTexts(
	master *HierarchyMaster,
	detectedTexts []string,
	folderName string,
	focusTarget string, // empty string means no focus target
) *HierarchyRow {
	keywords, workTypeHint, varietyHint := extractMatchKeywords(master, detectedTexts, folderName)
	excludeOther := shouldExcludeOtherForFolder(master, folderName)

	scored := scoreCandidates(master, keywords, workTypeHint, varietyHint, focusTarget, excludeOther)
	if len(scored) == 0 {
		return nil
	}

	sort.Slice(scored, func(i, j int) bool {
		return scored[i].score > scored[j].score
	})
	result := scored[0].row
	return &result
}

// FolderHasMasterEntry mirrors Rust's folder_has_master_entry.
// Returns true when:
//  1. folder name exactly equals any remarks field, or
//  2. folder name contains any search_pattern token, or
//  3. any master-known term is found in the folder name.
func FolderHasMasterEntry(master *HierarchyMaster, folderName string) bool {
	if folderName == "" {
		return false
	}
	// Check 1: exact remarks match.
	for _, row := range master.rows {
		if row.Remarks == folderName {
			return true
		}
	}
	// Check 2: search_pattern match.
	for _, row := range master.rows {
		if row.SearchPatterns == "" {
			continue
		}
		for _, pat := range splitPatterns(row.SearchPatterns) {
			if strings.Contains(folderName, pat) {
				return true
			}
		}
	}
	// Check 3: known terms.
	return len(extractKnownFolderTerms(master, folderName)) > 0
}

// DateToMonthDay converts "YYYY-MM-DD HH:MM:SS" (or EXIF / slash variants)
// into "M月D日". Mirrors Rust's date_to_month_day.
func DateToMonthDay(dateStr string) string {
	ymd := dateStr
	if i := strings.Index(dateStr, " "); i >= 0 {
		ymd = dateStr[:i]
	}
	// Accept '-', ':', '/' as YMD separators.
	sep := ""
	for _, c := range []string{"-", ":", "/"} {
		if strings.Contains(ymd, c) {
			sep = c
			break
		}
	}
	if sep == "" {
		return ""
	}
	parts := strings.SplitN(ymd, sep, 3)
	if len(parts) < 3 {
		return ""
	}
	month := 0
	day := 0
	fmt.Sscanf(parts[1], "%d", &month)
	fmt.Sscanf(parts[2], "%d", &day)
	if month >= 1 && month <= 12 && day >= 1 && day <= 31 {
		return fmt.Sprintf("%d月%d日", month, day)
	}
	return ""
}
