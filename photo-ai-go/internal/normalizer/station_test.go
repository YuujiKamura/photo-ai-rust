package normalizer

import (
	"testing"
)

// ---------------------------------------------------------------------------
// parse: empty/dash — mirrors parse_empty_string_returns_empty
// ---------------------------------------------------------------------------

func TestStationParseEmpty(t *testing.T) {
	for _, s := range []string{"", "  ", "-"} {
		st := StationParse(s)
		if !st.IsEmpty() {
			t.Errorf("StationParse(%q) kind=%v, want Empty", s, st.Kind())
		}
	}
}

// ---------------------------------------------------------------------------
// parse: Post without lane — mirrors parse_post_without_lane
// ---------------------------------------------------------------------------

func TestStationParsePostNoLane(t *testing.T) {
	st := StationParse("No.6")
	if !st.IsPost() {
		t.Fatalf("StationParse(\"No.6\") expected Post, got kind=%v", st.Kind())
	}
	p := st.Post()
	if p.Label != "No.6" {
		t.Errorf("label = %q, want No.6", p.Label)
	}
	if p.Lane != LaneNone {
		t.Errorf("lane = %v, want LaneNone", p.Lane)
	}
	if p.AccessRoad {
		t.Error("access_road should be false")
	}
}

// ---------------------------------------------------------------------------
// parse: Post with right lane — mirrors parse_post_with_right_lane
// ---------------------------------------------------------------------------

func TestStationParsePostRightLane(t *testing.T) {
	st := StationParse("No.6 R")
	if !st.IsPost() {
		t.Fatalf("expected Post")
	}
	p := st.Post()
	if p.Label != "No.6" {
		t.Errorf("label = %q, want No.6", p.Label)
	}
	if p.Lane != LaneRight {
		t.Errorf("lane = %v, want LaneRight", p.Lane)
	}
}

// ---------------------------------------------------------------------------
// parse: Post with left lane variants — mirrors parse_post_with_left_lane_variants
// ---------------------------------------------------------------------------

func TestStationParsePostLeftLaneVariants(t *testing.T) {
	for _, input := range []string{"No.4 L", "No.4 l", "No.4 左"} {
		st := StationParse(input)
		if !st.IsPost() {
			t.Errorf("StationParse(%q) expected Post", input)
			continue
		}
		if st.Post().Lane != LaneLeft {
			t.Errorf("StationParse(%q) lane = %v, want LaneLeft", input, st.Post().Lane)
		}
	}
}

// ---------------------------------------------------------------------------
// parse: access road — mirrors parse_access_road
// ---------------------------------------------------------------------------

func TestStationParseAccessRoad(t *testing.T) {
	st := StationParse("取付道路 No.1")
	if !st.IsPost() {
		t.Fatalf("expected Post")
	}
	p := st.Post()
	if p.Label != "No.1" {
		t.Errorf("label = %q, want No.1", p.Label)
	}
	if !p.AccessRoad {
		t.Error("access_road should be true")
	}
	if p.Lane != LaneNone {
		t.Errorf("lane = %v, want LaneNone", p.Lane)
	}
}

// ---------------------------------------------------------------------------
// parse: Date without dump — mirrors parse_date_without_dump
// ---------------------------------------------------------------------------

func TestStationParseDateNoDump(t *testing.T) {
	st := StationParse("2月11日")
	if !st.IsDate() {
		t.Fatalf("expected Date")
	}
	d := st.Date()
	if d.Month != 2 || d.Day != 11 {
		t.Errorf("date = %d月%d日, want 2月11日", d.Month, d.Day)
	}
	if d.DumpNumber != 0 {
		t.Errorf("dump_number = %d, want 0", d.DumpNumber)
	}
}

// ---------------------------------------------------------------------------
// parse: Date with dump number — mirrors parse_date_with_dump_number
// ---------------------------------------------------------------------------

func TestStationParseDateWithDump(t *testing.T) {
	st := StationParse("2月9日 1台目")
	if !st.IsDate() {
		t.Fatalf("expected Date")
	}
	d := st.Date()
	if d.Month != 2 || d.Day != 9 {
		t.Errorf("date = %d月%d日, want 2月9日", d.Month, d.Day)
	}
	if d.DumpNumber != 1 {
		t.Errorf("dump_number = %d, want 1", d.DumpNumber)
	}
}

// ---------------------------------------------------------------------------
// parse: Date rejects invalid month or day — mirrors parse_date_rejects_invalid_month_or_day
// ---------------------------------------------------------------------------

func TestStationParseDateRejectsInvalid(t *testing.T) {
	// 13月 → not Date
	st13 := StationParse("13月5日")
	if st13.IsDate() {
		t.Error("13月5日 should not parse as Date")
	}
	// 32日 → not Date
	st32 := StationParse("2月32日")
	if st32.IsDate() {
		t.Error("2月32日 should not parse as Date")
	}
}

// ---------------------------------------------------------------------------
// parse: Other — mirrors parse_line_type_falls_to_other
// ---------------------------------------------------------------------------

func TestStationParseOther(t *testing.T) {
	for _, input := range []string{"ダイヤモンド標示", "横断歩道線"} {
		st := StationParse(input)
		if st.Kind() != StationKindOther {
			t.Errorf("StationParse(%q) kind=%v, want Other", input, st.Kind())
		}
		if st.ToDisplay() != input {
			t.Errorf("ToDisplay() = %q, want %q", st.ToDisplay(), input)
		}
	}
}

// ---------------------------------------------------------------------------
// ToDisplay round-trip — mirrors roundtrip_* tests
// ---------------------------------------------------------------------------

func TestStationToDisplayRoundTrip(t *testing.T) {
	tests := []struct {
		input string
		want  string
	}{
		{"No.6", "No.6"},
		{"No.6 R", "No.6 R"},
		{"取付道路 No.1", "取付道路 No.1"},
		{"2月11日 2台目", "2月11日 2台目"},
	}
	for _, tt := range tests {
		got := StationParse(tt.input).ToDisplay()
		if got != tt.want {
			t.Errorf("StationParse(%q).ToDisplay() = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestStationEmptyToDisplayIsEmpty(t *testing.T) {
	st := Station{kind: StationKindEmpty}
	if got := st.ToDisplay(); got != "" {
		t.Errorf("Empty.ToDisplay() = %q, want \"\"", got)
	}
}

// ---------------------------------------------------------------------------
// WithDumpNumber — mirrors with_dump_number_* tests
// ---------------------------------------------------------------------------

func TestStationWithDumpNumberAddsToDate(t *testing.T) {
	st := StationParse("2月11日").WithDumpNumber(3)
	if st.ToDisplay() != "2月11日 3台目" {
		t.Errorf("got %q, want %q", st.ToDisplay(), "2月11日 3台目")
	}
}

func TestStationWithDumpNumberOverridesExisting(t *testing.T) {
	st := StationParse("2月11日 1台目").WithDumpNumber(5)
	if st.ToDisplay() != "2月11日 5台目" {
		t.Errorf("got %q, want %q", st.ToDisplay(), "2月11日 5台目")
	}
}

func TestStationWithDumpNumberNoEffectOnPost(t *testing.T) {
	before := StationParse("No.6 R")
	after := before.WithDumpNumber(3)
	if before.ToDisplay() != after.ToDisplay() {
		t.Errorf("WithDumpNumber on Post changed display: before=%q after=%q", before.ToDisplay(), after.ToDisplay())
	}
}

func TestStationWithDumpNumberNoEffectOnEmpty(t *testing.T) {
	st := Station{kind: StationKindEmpty}
	after := st.WithDumpNumber(2)
	if !after.IsEmpty() {
		t.Error("WithDumpNumber on Empty should return Empty")
	}
}

// ---------------------------------------------------------------------------
// Predicates — mirrors is_date_*, is_post_*, has_dump_number_*
// ---------------------------------------------------------------------------

func TestStationIsDateOnlyForDate(t *testing.T) {
	if !StationParse("2月11日").IsDate() {
		t.Error("2月11日 should be Date")
	}
	if StationParse("No.6").IsDate() {
		t.Error("No.6 should not be Date")
	}
	if (Station{kind: StationKindEmpty}).IsDate() {
		t.Error("Empty should not be Date")
	}
}

func TestStationIsPostForAccessRoadAndNormal(t *testing.T) {
	if !StationParse("No.6").IsPost() {
		t.Error("No.6 should be Post")
	}
	if !StationParse("取付道路 No.1").IsPost() {
		t.Error("取付道路 No.1 should be Post")
	}
	if StationParse("2月11日").IsPost() {
		t.Error("2月11日 should not be Post")
	}
}

func TestStationHasDumpNumberOnlyForDateWithDump(t *testing.T) {
	if !StationParse("2月11日 2台目").HasDumpNumber() {
		t.Error("2月11日 2台目 should have dump number")
	}
	if StationParse("2月11日").HasDumpNumber() {
		t.Error("2月11日 should not have dump number")
	}
	if StationParse("No.6 R").HasDumpNumber() {
		t.Error("No.6 R should not have dump number")
	}
	if (Station{kind: StationKindEmpty}).HasDumpNumber() {
		t.Error("Empty should not have dump number")
	}
}

func TestStationDumpNumberExtractsValue(t *testing.T) {
	if n := StationParse("2月11日 3台目").DumpNumber(); n != 3 {
		t.Errorf("DumpNumber() = %d, want 3", n)
	}
	if n := StationParse("2月11日").DumpNumber(); n != 0 {
		t.Errorf("DumpNumber() = %d, want 0", n)
	}
	if n := StationParse("No.6").DumpNumber(); n != 0 {
		t.Errorf("DumpNumber() = %d, want 0", n)
	}
}

// ---------------------------------------------------------------------------
// Golden tests with realistic Japanese strings (5 extra)
// ---------------------------------------------------------------------------

func TestStationParseGolden(t *testing.T) {
	tests := []struct {
		input   string
		wantFn  func(Station) bool
		display string
		desc    string
	}{
		{
			// typical construction station notation
			"No.3+20.5",
			func(s Station) bool { return s.Kind() == StationKindOther },
			"No.3+20.5",
			"non-integer post falls to Other",
		},
		{
			// temperature-management date with dump count
			"3月20日 5台目",
			func(s Station) bool { return s.IsDate() && s.Date().DumpNumber == 5 },
			"3月20日 5台目",
			"date with dump 5",
		},
		{
			// double-digit month + day
			"12月31日",
			func(s Station) bool {
				return s.IsDate() && s.Date().Month == 12 && s.Date().Day == 31
			},
			"12月31日",
			"Dec 31",
		},
		{
			// 大文字 NO. variant (case-insensitive)
			"NO.10",
			func(s Station) bool { return s.IsPost() && s.Post().Label == "No.10" },
			"No.10",
			"uppercase NO.",
		},
		{
			// Japanese freetext → Other (line-type name used in road-marking work)
			"外側線",
			func(s Station) bool { return s.Kind() == StationKindOther },
			"外側線",
			"line-type name → Other",
		},
	}
	for _, tt := range tests {
		st := StationParse(tt.input)
		if !tt.wantFn(st) {
			t.Errorf("[%s] StationParse(%q) predicate failed, kind=%v display=%q",
				tt.desc, tt.input, st.Kind(), st.ToDisplay())
		}
		if got := st.ToDisplay(); got != tt.display {
			t.Errorf("[%s] ToDisplay() = %q, want %q", tt.desc, got, tt.display)
		}
	}
}
