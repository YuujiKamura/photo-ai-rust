package normalizer

import (
	"math"
	"testing"
)

// -----------------------------------------------------------------------
// MeasurementExtractAll
// -----------------------------------------------------------------------

func TestMeasurementExtractAll_Temperature(t *testing.T) {
	ms := MeasurementExtractAll("出荷時156℃")
	if len(ms) != 1 {
		t.Fatalf("want 1 measurement, got %d", len(ms))
	}
	if ms[0].Kind != MeasurementKindTemperature {
		t.Errorf("want Temperature kind, got %v", ms[0].Kind)
	}
	if math.Abs(ms[0].Value-156.0) > 0.01 {
		t.Errorf("want 156.0, got %v", ms[0].Value)
	}
}

func TestMeasurementExtractAll_Dimension(t *testing.T) {
	ms := MeasurementExtractAll("t=50mm")
	if len(ms) != 1 {
		t.Fatalf("want 1 measurement, got %d", len(ms))
	}
	if ms[0].Kind != MeasurementKindDimension {
		t.Errorf("want Dimension kind, got %v", ms[0].Kind)
	}
	if math.Abs(ms[0].Value-50.0) > 0.01 {
		t.Errorf("want 50.0 mm, got %v", ms[0].Value)
	}
}

func TestMeasurementExtractAll_Mixed(t *testing.T) {
	// Mirrors test_extract_measurements in measurements.rs
	ms := MeasurementExtractAll("出荷時156℃、t=50mm")
	if len(ms) != 2 {
		t.Fatalf("want 2 measurements, got %d", len(ms))
	}
	if ms[0].Kind != MeasurementKindTemperature {
		t.Errorf("want first=Temperature, got %v", ms[0].Kind)
	}
	if ms[1].Kind != MeasurementKindDimension {
		t.Errorf("want second=Dimension, got %v", ms[1].Kind)
	}
}

func TestMeasurementExtractAll_Density(t *testing.T) {
	ms := MeasurementExtractAll("締固め度 98.5%")
	if len(ms) != 1 {
		t.Fatalf("want 1 measurement, got %d", len(ms))
	}
	if ms[0].Kind != MeasurementKindDensity {
		t.Errorf("want Density kind, got %v", ms[0].Kind)
	}
	if math.Abs(ms[0].Value-98.5) > 0.01 {
		t.Errorf("want 98.5, got %v", ms[0].Value)
	}
}

func TestMeasurementExtractAll_Empty(t *testing.T) {
	if ms := MeasurementExtractAll(""); len(ms) != 0 {
		t.Errorf("want empty, got %v", ms)
	}
}

// Golden tests — realistic strings
func TestMeasurementExtractAll_Golden(t *testing.T) {
	cases := []struct {
		input   string
		wantLen int
		wantKind MeasurementKind
		wantVal  float64
	}{
		{"厚さ 50mm", 1, MeasurementKindDimension, 50.0},
		{"幅 2.5m", 1, MeasurementKindDimension, 2500.0},
		{"密度 96%", 1, MeasurementKindDensity, 96.0},
		{"温度：158℃", 1, MeasurementKindTemperature, 158.0},
	}
	for _, c := range cases {
		ms := MeasurementExtractAll(c.input)
		if len(ms) < 1 {
			t.Errorf("input %q: want ≥1 measurement, got 0", c.input)
			continue
		}
		if ms[0].Kind != c.wantKind {
			t.Errorf("input %q: kind = %v, want %v", c.input, ms[0].Kind, c.wantKind)
		}
		if math.Abs(ms[0].Value-c.wantVal) > 0.01 {
			t.Errorf("input %q: value = %v, want %v", c.input, ms[0].Value, c.wantVal)
		}
	}
}

// -----------------------------------------------------------------------
// MeasurementTemperatureTypeFromText
// Mirrors test_temperature_type_from_text in measurements.rs
// -----------------------------------------------------------------------

func TestMeasurementTemperatureTypeFromText(t *testing.T) {
	cases := []struct {
		input string
		want  MeasurementTemperatureType
	}{
		{"到着温度", MeasurementTemperatureArrival},
		{"出荷時温度", MeasurementTemperatureArrival},
		{"出荷時", MeasurementTemperatureArrival},
		{"敷均し温度", MeasurementTemperatureSpreading},
		{"舗設温度", MeasurementTemperatureSpreading},
		{"初期締固め前温度", MeasurementTemperatureInitialCompaction},
		{"初期転圧前温度", MeasurementTemperatureInitialCompaction},
		{"開放温度", MeasurementTemperatureOpening},
		{"解放温度", MeasurementTemperatureOpening},
		{"温度測定", MeasurementTemperatureUnknown},
	}
	for _, c := range cases {
		got := MeasurementTemperatureTypeFromText(c.input)
		if got != c.want {
			t.Errorf("MeasurementTemperatureTypeFromText(%q) = %v, want %v", c.input, got, c.want)
		}
	}
}

// -----------------------------------------------------------------------
// MeasurementIsValidTemperature
// Mirrors test_temperature_type_valid_range in measurements.rs
// -----------------------------------------------------------------------

func TestMeasurementIsValidTemperature(t *testing.T) {
	cases := []struct {
		tt    MeasurementTemperatureType
		temp  float64
		valid bool
	}{
		{MeasurementTemperatureArrival, 160.0, true},
		{MeasurementTemperatureArrival, 155.0, true},
		{MeasurementTemperatureArrival, 50.0, false},  // too low
		{MeasurementTemperatureArrival, 200.0, false}, // too high
		{MeasurementTemperatureOpening, 50.0, true},
		{MeasurementTemperatureOpening, 32.6, true},
		{MeasurementTemperatureOpening, 126.0, false}, // too high
		{MeasurementTemperatureOpening, 20.0, false},  // too low
	}
	for _, c := range cases {
		got := MeasurementIsValidTemperature(c.tt, c.temp)
		if got != c.valid {
			t.Errorf("MeasurementIsValidTemperature(%v, %v) = %v, want %v", c.tt, c.temp, got, c.valid)
		}
	}
}

// -----------------------------------------------------------------------
// MeasurementValidateTemperature
// Mirrors test_validate_temperature_* in measurements.rs
// -----------------------------------------------------------------------

func TestMeasurementValidateTemperature_Valid(t *testing.T) {
	// Valid temperatures → no correction (empty string)
	cases := []struct {
		text string
		tt   MeasurementTemperatureType
	}{
		{"161.1℃", MeasurementTemperatureArrival},
		{"50.0℃", MeasurementTemperatureOpening},
		{"32.6℃", MeasurementTemperatureOpening},
	}
	for _, c := range cases {
		got := MeasurementValidateTemperature(c.text, c.tt)
		if got != "" {
			t.Errorf("MeasurementValidateTemperature(%q, %v) = %q, want empty", c.text, c.tt, got)
		}
	}
}

func TestMeasurementValidateTemperature_OpeningCorrection(t *testing.T) {
	// 126℃ for Opening is out-of-range → should produce a corrected value
	// that IS in range (either "1.26℃" or "12.6℃")
	result := MeasurementValidateTemperature("126℃", MeasurementTemperatureOpening)
	if result == "" {
		// If no correction was found, the test passes (same as Rust: correction may be None)
		t.Log("no correction produced for 126℃ Opening (acceptable)")
		return
	}
	correctedTemp, ok := ExtractTemperature(result)
	if !ok {
		t.Errorf("corrected value %q is not parseable as temperature", result)
	}
	if !MeasurementIsValidTemperature(MeasurementTemperatureOpening, correctedTemp) {
		t.Errorf("corrected temperature %v is not valid for Opening", correctedTemp)
	}
}

// -----------------------------------------------------------------------
// MeasurementExtractTemperatureForRemarks (full remarks-aware version)
// Mirrors test_extract_temperature_for_remarks_* in measurements.rs
// -----------------------------------------------------------------------

func TestMeasurementExtractTemperatureForRemarks_Named(t *testing.T) {
	cases := []struct {
		text    string
		remarks string
		want    string
	}{
		{
			"到着温度 159.7℃, ダンプ台数 1台目",
			"到着温度",
			"159.7℃",
		},
		{
			"到着温度 160.6℃, 敷均し温度 154.5℃, 初期転圧前温度 147.8℃",
			"敷均し温度",
			"154.5℃",
		},
		{
			"到着温度 160.6℃, 初期転圧前温度 147.8℃",
			"初期締固め前温度", // alias → 初期転圧前温度
			"147.8℃",
		},
		{
			"舗装日外気温 1℃, 解放温度 33.9℃",
			"開放温度", // alias → 解放温度
			"33.9℃",
		},
		{
			"舗装日外気温 1℃, 解放温度 33.9℃",
			"舗装日外気温",
			"1℃",
		},
	}
	for _, c := range cases {
		got := MeasurementExtractTemperatureForRemarks(c.text, c.remarks)
		if got != c.want {
			t.Errorf("ExtractTemperatureForRemarks(%q, %q) = %q, want %q", c.text, c.remarks, got, c.want)
		}
	}
}

func TestMeasurementExtractTemperatureForRemarks_NumberOnly(t *testing.T) {
	// Mirrors test_extract_temperature_for_remarks_number_only
	cases := []struct {
		text    string
		remarks string
		want    string
	}{
		{"159.7℃", "到着温度", "159.7℃"},
		{"148.1", "初期締固め前温度", "148.1℃"},
		{"33.9℃", "開放温度", "33.9℃"},
	}
	for _, c := range cases {
		got := MeasurementExtractTemperatureForRemarks(c.text, c.remarks)
		if got != c.want {
			t.Errorf("ExtractTemperatureForRemarks(%q, %q) = %q, want %q", c.text, c.remarks, got, c.want)
		}
	}
}

func TestMeasurementExtractTemperatureForRemarks_NoMatch(t *testing.T) {
	// Mirrors test_extract_temperature_for_remarks_no_match
	// Multi-value weather app text — should NOT match for Opening
	got := MeasurementExtractTemperatureForRemarks(
		"今日2/10(火), くもり のち 雨, 80%, 11℃, -1℃",
		"開放温度",
	)
	if got != "" {
		t.Errorf("expected empty, got %q", got)
	}

	// Empty text
	if got := MeasurementExtractTemperatureForRemarks("", "到着温度"); got != "" {
		t.Errorf("expected empty for empty text, got %q", got)
	}
}

// -----------------------------------------------------------------------
// Golden tests — realistic field strings (additional, not in Rust)
// -----------------------------------------------------------------------

func TestMeasurementGoldenStrings(t *testing.T) {
	// "厚さ 50mm" → dimension 50 mm
	ms := MeasurementExtractAll("厚さ 50mm")
	if len(ms) == 0 || ms[0].Kind != MeasurementKindDimension || math.Abs(ms[0].Value-50) > 0.01 {
		t.Errorf("golden厚さ50mm failed: %+v", ms)
	}

	// "幅 2.5m" → dimension 2500 mm
	ms = MeasurementExtractAll("幅 2.5m")
	if len(ms) == 0 || ms[0].Kind != MeasurementKindDimension || math.Abs(ms[0].Value-2500) > 0.01 {
		t.Errorf("golden幅2.5m failed: %+v", ms)
	}

	// "出来形 良好" → no measurement
	ms = MeasurementExtractAll("出来形 良好")
	if len(ms) != 0 {
		t.Errorf("golden出来形良好: expected 0 measurements, got %d", len(ms))
	}

	// "管理基準値 ±10mm" — RE2 has no lookahead; ± is literal, regex matches "10mm"
	ms = MeasurementExtractAll("管理基準値 ±10mm")
	if len(ms) == 0 {
		t.Errorf("golden管理基準値±10mm: expected ≥1 measurement, got 0")
	}
}
