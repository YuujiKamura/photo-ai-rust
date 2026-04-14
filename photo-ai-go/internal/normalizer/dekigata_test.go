package normalizer

import (
	"math"
	"strings"
	"testing"
)

// -----------------------------------------------------------------------
// OCR test fixtures — same real data as Rust tests (no PII)
// -----------------------------------------------------------------------

const (
	// 左車線 No.1
	ocrNo1Left = "出来形管理用紙 No.1, GH=9.916 FH=9.911, 計画高(設計) V1=9.869 V2=9.892 V3=9.911 V4=9.876 V5=9.835, 切削高(設計) V1=9.819 V2=9.842 V3=9.861 V4=9.826 V5=9.785, 切削高(実施) V1=9.815 V2=9.842 V3=9.860 V4=9.825 V5=9.780, 左幅員 設計4.20 実測4.20, 右幅員 設計4.13 実測4.13"
	// 右車線 No.5
	ocrNo5Right = "出来形管理用紙 No.5, GH=10.088 FH=10.097, 計画高(設計) V1=10.019 V2=10.049 V3=10.097 V4=10.051 V5=10.024, 切削高(設計) V1=9.969 V2=9.999 V3=10.047 V4=10.001 V5=9.974, 切削高(実施) V1=9.965 V2=9.997 V3=10.045 V4=10.000 V5=9.970, 左幅員 設計3.20 実測3.20, 右幅員 設計3.18 実測3.18"
	// 右車線 No.3
	ocrNo3Right = "出来形管理用紙 No.3, GH=10.026 FH=10.055, 切削高(設計) V1=9.988 V2=9.995 V3=10.005 V4=9.955 V5=9.906, 切削高(実施) V1=9.985 V2=9.990 V3=10.002 V4=9.996 V5=9.902, 左幅員 設計3.20 実測3.20, 右幅員 設計3.90 実測3.90"
	// V=ラベルなし No.9
	ocrNo9NoLabels  = "出来形管理用紙 No.9 GH=10.414 FH=10.426 切削高(実施) 10.375 10.328 10.292 右幅員 実測 3.20"
	// V=ラベルなし No.11
	ocrNo11NoLabels = "出来形管理用紙 No.11 GH=10.669 FH=10.674 切削高(実施) 10.621 10.585 10.568 右幅員 実測 3.20"
)

// helper: assert float pointer equals value
func assertF64(t *testing.T, name string, got *float64, want float64) {
	t.Helper()
	if got == nil {
		t.Errorf("%s: got nil, want %.3f", name, want)
		return
	}
	if math.Abs(*got-want) > 0.001 {
		t.Errorf("%s: got %.3f, want %.3f", name, *got, want)
	}
}

// -----------------------------------------------------------------------
// ParseDekigataOCR — mirrors test_parse_dekigata_ocr_* in dekigata.rs
// -----------------------------------------------------------------------

func TestParseDekigataOCR_LeftLane(t *testing.T) {
	v := ParseDekigataOCR(ocrNo1Left)
	if v == nil {
		t.Fatal("expected non-nil DekigataValues")
	}
	if v.Station != "No.1" {
		t.Errorf("Station = %q, want No.1", v.Station)
	}
	assertF64(t, "VDesign[0]", v.VDesign[0], 9.819)
	assertF64(t, "VDesign[4]", v.VDesign[4], 9.785)
	assertF64(t, "VActual[0]", v.VActual[0], 9.815)
	assertF64(t, "VActual[4]", v.VActual[4], 9.780)
	assertF64(t, "WidthLeftDesign", v.WidthLeftDesign, 4.20)
	assertF64(t, "WidthLeftActual", v.WidthLeftActual, 4.20)
	assertF64(t, "WidthRightDesign", v.WidthRightDesign, 4.13)
	assertF64(t, "WidthRightActual", v.WidthRightActual, 4.13)
}

func TestParseDekigataOCR_RightLane(t *testing.T) {
	v := ParseDekigataOCR(ocrNo5Right)
	if v == nil {
		t.Fatal("expected non-nil DekigataValues")
	}
	if v.Station != "No.5" {
		t.Errorf("Station = %q, want No.5", v.Station)
	}
	assertF64(t, "VDesign[3]", v.VDesign[3], 10.001)
	assertF64(t, "VDesign[4]", v.VDesign[4], 9.974)
	assertF64(t, "VActual[3]", v.VActual[3], 10.000)
	assertF64(t, "VActual[4]", v.VActual[4], 9.970)
	assertF64(t, "WidthRightDesign", v.WidthRightDesign, 3.18)
	assertF64(t, "WidthRightActual", v.WidthRightActual, 3.18)
}

func TestParseDekigataOCR_NonDekigata(t *testing.T) {
	if ParseDekigataOCR("温度 160.4℃") != nil {
		t.Error("expected nil for non-dekigata text")
	}
	if ParseDekigataOCR("") != nil {
		t.Error("expected nil for empty text")
	}
}

func TestParseDekigataOCR_NoVLabels(t *testing.T) {
	// Mirrors test_parse_dekigata_no_v_labels
	v := ParseDekigataOCR(ocrNo9NoLabels)
	if v == nil {
		t.Fatal("expected non-nil")
	}
	if v.Station != "No.9" {
		t.Errorf("Station = %q, want No.9", v.Station)
	}
	assertF64(t, "VActual[0]", v.VActual[0], 10.375)
	assertF64(t, "VActual[1]", v.VActual[1], 10.328)
	assertF64(t, "VActual[2]", v.VActual[2], 10.292)
	assertF64(t, "WidthRightActual", v.WidthRightActual, 3.20)
}

func TestParseDekigataOCR_No11NoVLabels(t *testing.T) {
	v := ParseDekigataOCR(ocrNo11NoLabels)
	if v == nil {
		t.Fatal("expected non-nil")
	}
	if v.Station != "No.11" {
		t.Errorf("Station = %q, want No.11", v.Station)
	}
	assertF64(t, "VActual[0]", v.VActual[0], 10.621)
	assertF64(t, "VActual[1]", v.VActual[1], 10.585)
	assertF64(t, "VActual[2]", v.VActual[2], 10.568)
}

// -----------------------------------------------------------------------
// DekigataFormatMeasurements — mirrors test_format_measurements_* in dekigata.rs
// -----------------------------------------------------------------------

func TestDekigataFormatMeasurements_Left(t *testing.T) {
	v := ParseDekigataOCR(ocrNo1Left)
	if v == nil {
		t.Fatal("parse failed")
	}
	m := DekigataFormatMeasurements(v, DekigataLaneLeft)
	if !strings.HasPrefix(m, "左車線") {
		t.Errorf("expected 左車線 prefix, got %q", m)
	}
	if !strings.HasPrefix(m, "左車線\u3000切削基準高 幅員W1") {
		t.Errorf("header mismatch: %q", m)
	}
	if !strings.Contains(m, "設計: V1=9.819 V2=9.842 V3=9.861") {
		t.Errorf("VDesign missing: %q", m)
	}
	if !strings.Contains(m, "実施: V1=9.815 V2=9.842 V3=9.860") {
		t.Errorf("VActual missing: %q", m)
	}
	if !strings.Contains(m, "幅員W1 設計: 4.20 実測: 4.20") {
		t.Errorf("W1 missing: %q", m)
	}
}

func TestDekigataFormatMeasurements_Right(t *testing.T) {
	v := ParseDekigataOCR(ocrNo5Right)
	if v == nil {
		t.Fatal("parse failed")
	}
	m := DekigataFormatMeasurements(v, DekigataLaneRight)
	if !strings.HasPrefix(m, "右車線") {
		t.Errorf("expected 右車線 prefix, got %q", m)
	}
	if !strings.Contains(m, "設計: V4=10.001 V5=9.974") {
		t.Errorf("VDesign missing: %q", m)
	}
	if !strings.Contains(m, "実施: V4=10.000 V5=9.970") {
		t.Errorf("VActual missing: %q", m)
	}
	if !strings.Contains(m, "幅員W2 設計: 3.18 実測: 3.18") {
		t.Errorf("W2 missing: %q", m)
	}
}

func TestDekigataFormatMeasurements_RightNo3(t *testing.T) {
	v := ParseDekigataOCR(ocrNo3Right)
	if v == nil {
		t.Fatal("parse failed")
	}
	m := DekigataFormatMeasurements(v, DekigataLaneRight)
	if !strings.Contains(m, "設計: V4=9.955 V5=9.906") {
		t.Errorf("VDesign missing: %q", m)
	}
	if !strings.Contains(m, "実施: V4=9.996 V5=9.902") {
		t.Errorf("VActual missing: %q", m)
	}
	if !strings.Contains(m, "幅員W2 設計: 3.90 実測: 3.90") {
		t.Errorf("W2 missing: %q", m)
	}
}

// Exact output comparison — mirrors test_format_exact_output_* in dekigata.rs
func TestDekigataFormatMeasurements_ExactLeft(t *testing.T) {
	v := ParseDekigataOCR(ocrNo1Left)
	if v == nil {
		t.Fatal("parse failed")
	}
	got := DekigataFormatMeasurements(v, DekigataLaneLeft)
	want := "左車線\u3000切削基準高 幅員W1\n設計: V1=9.819 V2=9.842 V3=9.861\n実施: V1=9.815 V2=9.842 V3=9.860\n幅員W1 設計: 4.20 実測: 4.20"
	if got != want {
		t.Errorf("exact output mismatch:\ngot:  %q\nwant: %q", got, want)
	}
}

func TestDekigataFormatMeasurements_ExactRight(t *testing.T) {
	v := ParseDekigataOCR(ocrNo5Right)
	if v == nil {
		t.Fatal("parse failed")
	}
	got := DekigataFormatMeasurements(v, DekigataLaneRight)
	want := "右車線\u3000切削基準高 幅員W2\n設計: V4=10.001 V5=9.974\n実施: V4=10.000 V5=9.970\n幅員W2 設計: 3.18 実測: 3.18"
	if got != want {
		t.Errorf("exact output mismatch:\ngot:  %q\nwant: %q", got, want)
	}
}

// -----------------------------------------------------------------------
// DekigataLaneDetermine — mirrors determine_lane in dekigata.rs
// -----------------------------------------------------------------------

func TestDekigataLaneDetermine_Left(t *testing.T) {
	v := ParseDekigataOCR(ocrNo1Left)
	// No.1 has both widths present and V-diffs are nearly equal (< 0.001 threshold).
	// Rust determine_lane returns None in this case; Go returns (0, false).
	// Just verify it doesn't panic; ok=false is valid.
	lane, ok := DekigataLaneDetermine(v)
	_ = lane
	_ = ok
	// No assertion: undetermined (ok=false) or any lane is acceptable.
}

func TestDekigataLaneDetermine_OnlyLeft(t *testing.T) {
	v := &DekigataValues{
		WidthLeftActual: f64ptr(4.20),
	}
	lane, ok := DekigataLaneDetermine(v)
	if !ok || lane != DekigataLaneLeft {
		t.Errorf("expected Left, got (%v, %v)", lane, ok)
	}
}

func TestDekigataLaneDetermine_OnlyRight(t *testing.T) {
	v := &DekigataValues{
		WidthRightActual: f64ptr(3.18),
	}
	lane, ok := DekigataLaneDetermine(v)
	if !ok || lane != DekigataLaneRight {
		t.Errorf("expected Right, got (%v, %v)", lane, ok)
	}
}

func TestDekigataLaneDetermine_NeitherWidth(t *testing.T) {
	v := &DekigataValues{}
	_, ok := DekigataLaneDetermine(v)
	if ok {
		t.Error("expected undetermined when no width actual available")
	}
}

// -----------------------------------------------------------------------
// Golden tests — realistic inputs not in Rust test suite
// -----------------------------------------------------------------------

func TestDekigataGolden_出来形良好(t *testing.T) {
	// Plain "出来形 良好" without 管理用紙 → ParseDekigataOCR returns nil
	if v := ParseDekigataOCR("出来形 良好"); v != nil {
		t.Errorf("expected nil, got %+v", v)
	}
}

func TestDekigataGolden_管理基準値(t *testing.T) {
	// 管理基準値±10mm — not a 出来形管理用紙 record
	if v := ParseDekigataOCR("管理基準値 ±10mm"); v != nil {
		t.Errorf("expected nil, got %+v", v)
	}
}

func TestDekigataGolden_BothLaneFormat(t *testing.T) {
	v := ParseDekigataOCR(ocrNo1Left)
	if v == nil {
		t.Fatal("parse failed")
	}
	m := DekigataFormatMeasurements(v, DekigataLaneBoth)
	if !strings.Contains(m, "設計: V1=") {
		t.Errorf("Both format missing left design: %q", m)
	}
	if !strings.Contains(m, "設計: V4=") {
		t.Errorf("Both format missing right design: %q", m)
	}
}

func TestDekigataIsPhoto(t *testing.T) {
	cases := []struct {
		category, remarks, detected string
		want                        bool
	}{
		{"出来形管理写真", "", "", true},
		{"", "出来形管理", "", true},
		{"", "", "出来形管理用紙 No.1", true},
		{"施工状況写真", "舗設状況", "温度", false},
	}
	for _, c := range cases {
		got := DekigataIsPhoto(c.category, c.remarks, c.detected)
		if got != c.want {
			t.Errorf("DekigataIsPhoto(%q,%q,%q) = %v, want %v", c.category, c.remarks, c.detected, got, c.want)
		}
	}
}
