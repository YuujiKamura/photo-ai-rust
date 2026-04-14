package normalizer

import (
	"strings"
	"testing"
)

// ---------------------------------------------------------------------------
// TemperatureLabel — mirrors label_has_measurement_suffix
// ---------------------------------------------------------------------------

func TestTemperatureLabelHasMeasurementSuffix(t *testing.T) {
	for _, k := range TemperatureAllKinds() {
		label := k.TemperatureLabel()
		if !strings.HasSuffix(label, "測定") {
			t.Errorf("TemperatureKind(%d).TemperatureLabel() = %q, want suffix 測定", k, label)
		}
	}
}

// ---------------------------------------------------------------------------
// TemperatureShortLabel — mirrors short_label_has_no_measurement_suffix
// ---------------------------------------------------------------------------

func TestTemperatureShortLabelHasNoSuffix(t *testing.T) {
	for _, k := range TemperatureAllKinds() {
		short := k.TemperatureShortLabel()
		if strings.HasSuffix(short, "測定") {
			t.Errorf("TemperatureKind(%d).TemperatureShortLabel() = %q, must NOT have suffix 測定", k, short)
		}
	}
}

// ---------------------------------------------------------------------------
// TemperatureShortLabel + TemperatureLabel relationship
// mirrors short_label_is_prefix_of_label
// ---------------------------------------------------------------------------

func TestTemperatureShortLabelIsPrefixOfLabel(t *testing.T) {
	for _, k := range TemperatureAllKinds() {
		want := k.TemperatureShortLabel() + "測定"
		got := k.TemperatureLabel()
		if got != want {
			t.Errorf("TemperatureKind(%d): label=%q, want %q", k, got, want)
		}
	}
}

// ---------------------------------------------------------------------------
// TemperatureKindFromText — mirrors from_text_detects_canonical_labels
// ---------------------------------------------------------------------------

func TestTemperatureKindFromTextCanonical(t *testing.T) {
	tests := []struct {
		input string
		want  TemperatureKind
	}{
		{"到着温度", TemperatureArrival},
		{"敷均し温度", TemperatureSpreading},
		{"初期締固め前温度", TemperatureInitialCompaction},
		{"開放温度", TemperatureOpening},
		{"舗装日外気温", TemperatureOutsideAir},
	}
	for _, tt := range tests {
		got, ok := TemperatureKindFromText(tt.input)
		if !ok {
			t.Errorf("TemperatureKindFromText(%q) = _, false; want true", tt.input)
			continue
		}
		if got != tt.want {
			t.Errorf("TemperatureKindFromText(%q) = %d, want %d", tt.input, got, tt.want)
		}
	}
}

// ---------------------------------------------------------------------------
// TemperatureKindFromText — mirrors from_text_handles_spelling_variants
// ---------------------------------------------------------------------------

func TestTemperatureKindFromTextSpellingVariants(t *testing.T) {
	tests := []struct {
		input string
		want  TemperatureKind
	}{
		// 初期転圧前 (旧表記) → InitialCompaction
		{"初期転圧前温度", TemperatureInitialCompaction},
		// 解放温度 (誤字) → Opening
		{"解放温度 45℃", TemperatureOpening},
		// 外気温単独 → OutsideAir
		{"外気温 12℃", TemperatureOutsideAir},
		// 敷きならし (ひらがな表記) → Spreading
		{"敷きならし直後", TemperatureSpreading},
	}
	for _, tt := range tests {
		got, ok := TemperatureKindFromText(tt.input)
		if !ok {
			t.Errorf("TemperatureKindFromText(%q) = _, false; want true", tt.input)
			continue
		}
		if got != tt.want {
			t.Errorf("TemperatureKindFromText(%q) = %d, want %d", tt.input, got, tt.want)
		}
	}
}

// ---------------------------------------------------------------------------
// TemperatureKindFromText — mirrors from_text_returns_none_for_unrelated
// ---------------------------------------------------------------------------

func TestTemperatureKindFromTextUnrelated(t *testing.T) {
	cases := []string{"", "舗設状況", "密度試験"}
	for _, s := range cases {
		if _, ok := TemperatureKindFromText(s); ok {
			t.Errorf("TemperatureKindFromText(%q) = _, true; want false", s)
		}
	}
}

// ---------------------------------------------------------------------------
// TemperatureIsValid — mirrors valid_range_arrival_covers_typical_values
// ---------------------------------------------------------------------------

func TestTemperatureIsValidArrival(t *testing.T) {
	k := TemperatureArrival
	if !TemperatureIsValid(k, 160.0) {
		t.Error("expected 160.0 valid for Arrival")
	}
	if !TemperatureIsValid(k, 140.0) { // boundary
		t.Error("expected 140.0 valid for Arrival (lower boundary)")
	}
	if !TemperatureIsValid(k, 185.0) { // boundary
		t.Error("expected 185.0 valid for Arrival (upper boundary)")
	}
	if TemperatureIsValid(k, 139.9) {
		t.Error("expected 139.9 invalid for Arrival")
	}
	if TemperatureIsValid(k, 185.1) {
		t.Error("expected 185.1 invalid for Arrival")
	}
}

// ---------------------------------------------------------------------------
// TemperatureIsValid — mirrors valid_range_opening_rejects_misread_ocr
// ---------------------------------------------------------------------------

func TestTemperatureIsValidOpening(t *testing.T) {
	k := TemperatureOpening
	if !TemperatureIsValid(k, 32.6) {
		t.Error("expected 32.6 valid for Opening")
	}
	if !TemperatureIsValid(k, 45.0) {
		t.Error("expected 45.0 valid for Opening")
	}
	if TemperatureIsValid(k, 126.0) { // OCR misread pattern
		t.Error("expected 126.0 invalid for Opening")
	}
	if TemperatureIsValid(k, 0.0) {
		t.Error("expected 0.0 invalid for Opening")
	}
}

// ---------------------------------------------------------------------------
// TemperatureIsValid — mirrors valid_range_outside_air_allows_below_zero
// ---------------------------------------------------------------------------

func TestTemperatureIsValidOutsideAir(t *testing.T) {
	k := TemperatureOutsideAir
	if !TemperatureIsValid(k, -5.0) {
		t.Error("expected -5.0 valid for OutsideAir")
	}
	if !TemperatureIsValid(k, 30.0) {
		t.Error("expected 30.0 valid for OutsideAir")
	}
	if TemperatureIsValid(k, -20.0) {
		t.Error("expected -20.0 invalid for OutsideAir")
	}
	if TemperatureIsValid(k, 60.0) {
		t.Error("expected 60.0 invalid for OutsideAir")
	}
}

// ---------------------------------------------------------------------------
// TemperatureIsValid — mirrors valid_range_initial_compaction_between_spreading_and_opening
// ---------------------------------------------------------------------------

func TestTemperatureIsValidInitialCompactionRangeOrdering(t *testing.T) {
	_, maxIC := TemperatureValidRange(TemperatureInitialCompaction)
	_, maxSP := TemperatureValidRange(TemperatureSpreading)
	minOP, _ := TemperatureValidRange(TemperatureOpening)
	minIC, _ := TemperatureValidRange(TemperatureInitialCompaction)

	// Temperature order: Spreading > InitialCompaction > Opening
	if maxIC >= maxSP {
		t.Errorf("expected max(InitialCompaction)=%v < max(Spreading)=%v", maxIC, maxSP)
	}
	if minIC <= minOP {
		t.Errorf("expected min(InitialCompaction)=%v > min(Opening)=%v", minIC, minOP)
	}
}

// ---------------------------------------------------------------------------
// TemperatureAllKinds — mirrors all_returns_five_kinds
// ---------------------------------------------------------------------------

func TestTemperatureAllKindsFiveElements(t *testing.T) {
	if n := len(TemperatureAllKinds()); n != 5 {
		t.Errorf("TemperatureAllKinds() len = %d, want 5", n)
	}
}

// ---------------------------------------------------------------------------
// TemperatureAllLabels — mirrors all_labels_contains_both_suffix_styles
// ---------------------------------------------------------------------------

func TestTemperatureAllLabelsContainsBothStyles(t *testing.T) {
	labels := TemperatureAllLabels()
	if len(labels) != 9 {
		t.Errorf("TemperatureAllLabels() len = %d, want 9", len(labels))
	}
	has := func(target string) bool {
		for _, l := range labels {
			if l == target {
				return true
			}
		}
		return false
	}
	if !has("到着温度測定") {
		t.Error("missing 到着温度測定")
	}
	if !has("到着温度") {
		t.Error("missing 到着温度")
	}
}

// ---------------------------------------------------------------------------
// Golden tests with realistic Japanese strings (5 extra)
// ---------------------------------------------------------------------------

func TestTemperatureKindFromTextGolden(t *testing.T) {
	tests := []struct {
		input   string
		wantOK  bool
		wantKnd TemperatureKind
	}{
		// "到着温度 165℃" — board text with value
		{"到着温度 165℃", true, TemperatureArrival},
		// "敷均し温度 155.5 度" — spreading board
		{"敷均し温度 155.5 度", true, TemperatureSpreading},
		// Full-width label with trailing 「測定」
		{"初期締固め前温度測定", true, TemperatureInitialCompaction},
		// 解放温度 (common misspelling on boards)
		{"解放温度 42℃", true, TemperatureOpening},
		// 外気温のみ (ambient air abbreviated)
		{"外気温 8℃", true, TemperatureOutsideAir},
	}
	for _, tt := range tests {
		got, ok := TemperatureKindFromText(tt.input)
		if ok != tt.wantOK {
			t.Errorf("TemperatureKindFromText(%q) ok=%v, want %v", tt.input, ok, tt.wantOK)
			continue
		}
		if ok && got != tt.wantKnd {
			t.Errorf("TemperatureKindFromText(%q) kind=%d, want %d", tt.input, got, tt.wantKnd)
		}
	}
}
