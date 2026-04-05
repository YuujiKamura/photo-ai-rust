package matching

import (
	"testing"
)

// TestBuildBigrams verifies bigram extraction from Japanese text.
func TestBuildBigrams(t *testing.T) {
	runes := []rune("アスファルト")
	bg := buildBigrams(runes)

	expected := []string{
		bigramKey('ア', 'ス'),
		bigramKey('ス', 'フ'),
		bigramKey('フ', 'ァ'),
		bigramKey('ァ', 'ル'),
		bigramKey('ル', 'ト'),
	}
	for _, k := range expected {
		if _, ok := bg[k]; !ok {
			t.Errorf("expected bigram %q not found in bigrams of %q", k, "アスファルト")
		}
	}
	if len(bg) != len(expected) {
		t.Errorf("expected %d bigrams, got %d", len(expected), len(bg))
	}
}

// TestBigramSimilarityIdentical verifies that identical strings score 1.0.
func TestBigramSimilarityIdentical(t *testing.T) {
	s := []rune("舗装工")
	bg := buildBigrams(s)
	score := bigramSimilarity(bg, bg)
	if score != 1.0 {
		t.Errorf("identical strings should score 1.0, got %f", score)
	}
}

// TestBigramSimilarityEmpty verifies that empty strings score 0.0.
func TestBigramSimilarityEmpty(t *testing.T) {
	empty := buildBigrams([]rune(""))
	other := buildBigrams([]rune("舗装工"))
	if bigramSimilarity(empty, other) != 0.0 {
		t.Error("empty vs non-empty should score 0.0")
	}
	if bigramSimilarity(empty, empty) != 0.0 {
		t.Error("empty vs empty should score 0.0")
	}
}

// TestBigramSimilarityJapanese checks similarity between related Japanese terms.
func TestBigramSimilarityJapanese(t *testing.T) {
	tests := []struct {
		a, b    string
		wantMin float64 // minimum expected similarity
		wantMax float64 // maximum expected similarity
	}{
		// Same string → 1.0
		{"アスファルト舗装", "アスファルト舗装", 1.0, 1.0},
		// Highly similar
		{"アスファルト舗装工", "アスファルト舗装", 0.7, 1.0},
		// Partially overlapping construction terms
		{"路盤工", "路盤", 0.4, 1.0},
		// Unrelated terms should be low
		{"舗装工", "排水工", 0.0, 0.4},
		// Single rune → 0 bigrams each
		{"工", "工", 0.0, 0.0},
	}

	for _, tt := range tests {
		a := buildBigrams([]rune(tt.a))
		b := buildBigrams([]rune(tt.b))
		got := bigramSimilarity(a, b)
		if got < tt.wantMin || got > tt.wantMax {
			t.Errorf("bigramSimilarity(%q, %q) = %f, want [%f, %f]",
				tt.a, tt.b, got, tt.wantMin, tt.wantMax)
		}
	}
}

// TestNormalizeForMatch checks normalization of fullwidth and whitespace.
func TestNormalizeForMatch(t *testing.T) {
	tests := []struct {
		input string
		want  string
	}{
		// Fullwidth ASCII → halfwidth
		{"アスファルト　舗装", "アスファルト舗装"}, // ideographic space stripped
		{"ＡＢＣＤ", "abcd"},          // fullwidth ASCII → halfwidth + lower
		{"  舗装工  ", "舗装工"},        // leading/trailing space
	}
	for _, tt := range tests {
		got := normalizeForMatch(tt.input)
		if got != tt.want {
			t.Errorf("normalizeForMatch(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

// TestMatcherMatch verifies the Matcher finds correct entries for Japanese queries.
func TestMatcherMatch(t *testing.T) {
	entries := []MasterEntry{
		{WorkType: "舗装工", Variety: "アスファルト舗装", Subphase: "路盤工"},
		{WorkType: "舗装工", Variety: "アスファルト舗装", Subphase: "基層工"},
		{WorkType: "舗装工", Variety: "アスファルト舗装", Subphase: "表層工"},
		{WorkType: "排水工", Variety: "側溝工", Subphase: ""},
		{WorkType: "法面工", Variety: "植生工", Subphase: ""},
	}
	m := NewMatcher(entries)

	// Should match 舗装工 entries
	result, ok := m.Match("アスファルト舗装 表層工", "施工状況")
	if !ok {
		t.Fatal("expected a match for 表層工 query, got none")
	}
	if result.Entry.WorkType != "舗装工" {
		t.Errorf("expected WorkType=舗装工, got %q", result.Entry.WorkType)
	}
	if result.Confidence <= 0.0 || result.Confidence > 1.0 {
		t.Errorf("confidence out of range: %f", result.Confidence)
	}

	// Query about 排水 should not match 舗装
	result2, ok2 := m.Match("排水 側溝", "")
	if ok2 && result2.Entry.WorkType == "舗装工" {
		t.Error("排水 query should not match 舗装工 as best")
	}
}

// TestMatcherEmpty verifies that an empty Matcher returns no match.
func TestMatcherEmpty(t *testing.T) {
	m := NewMatcher(nil)
	_, ok := m.Match("舗装工", "")
	if ok {
		t.Error("empty Matcher should return no match")
	}
}

// TestMatchAll verifies ranking order.
func TestMatchAll(t *testing.T) {
	entries := []MasterEntry{
		{WorkType: "舗装工", Variety: "アスファルト舗装", Subphase: "表層工"},
		{WorkType: "舗装工", Variety: "アスファルト舗装", Subphase: "路盤工"},
		{WorkType: "排水工", Variety: "側溝工", Subphase: ""},
	}
	m := NewMatcher(entries)
	results := m.MatchAll("アスファルト舗装 表層", "")
	if len(results) == 0 {
		t.Fatal("expected at least one result")
	}
	// Results must be sorted descending by confidence.
	for i := 1; i < len(results); i++ {
		if results[i].Confidence > results[i-1].Confidence {
			t.Errorf("results not sorted: index %d (%f) > index %d (%f)",
				i, results[i].Confidence, i-1, results[i-1].Confidence)
		}
	}
}

// TestScoreHierarchy verifies weighted scoring across 工種/種別/細別.
func TestScoreHierarchy(t *testing.T) {
	e := MasterEntry{WorkType: "舗装工", Variety: "アスファルト舗装", Subphase: "表層工"}

	scoreHigh := ScoreHierarchy("舗装工 アスファルト舗装 表層工", e)
	scoreLow := ScoreHierarchy("排水工 側溝", e)

	if scoreHigh <= scoreLow {
		t.Errorf("matching query should score higher: got high=%f low=%f", scoreHigh, scoreLow)
	}
	if scoreHigh < 0.3 {
		t.Errorf("matching query score too low: %f", scoreHigh)
	}
}

// TestMatchWorkType verifies two-stage hierarchy matching.
func TestMatchWorkType(t *testing.T) {
	entries := []MasterEntry{
		{WorkType: "舗装工", Variety: "アスファルト舗装", Subphase: "表層工"},
		{WorkType: "舗装工", Variety: "アスファルト舗装", Subphase: "路盤工"},
		{WorkType: "排水工", Variety: "側溝工", Subphase: ""},
	}
	m := NewMatcher(entries)

	// Filter to 舗装工 first, then find best 種別/細別 match.
	result, ok := m.MatchWorkType("舗装工", "表層工 施工")
	if !ok {
		t.Fatal("MatchWorkType should find a match within 舗装工")
	}
	if result.Entry.WorkType != "舗装工" {
		t.Errorf("expected 舗装工, got %q", result.Entry.WorkType)
	}

	// Non-existent work type → no match.
	_, ok2 := m.MatchWorkType("存在しない工種", "なにか")
	if ok2 {
		t.Error("non-existent WorkType should return no match")
	}
}
