package analyzer

// Tests port common/src/analyzer.rs #[cfg(test)] mod tests verbatim.
// 7 Rust test cases → 7 Go test cases (1:1 mapping).
// Table-driven style groups inputs; each sub-test corresponds to one Rust test fn.

import (
	"strings"
	"testing"

	"github.com/YuujiKamura/photo-ai-go/internal/ai"
)

// helper: build a minimal ai.RawImageData with only the fields relevant to
// the analyzer. Mirrors: RawImageData { file_name, photo_category,
// scene_description, detected_text, ..Default::default() }
func rawData(fileName, photoCategory, detectedText, sceneDescription string) ai.RawImageData {
	return ai.RawImageData{
		FileName:         fileName,
		PhotoCategory:    photoCategory,
		DetectedText:     detectedText,
		SceneDescription: sceneDescription,
	}
}

// contains reports whether slice s contains elem.
func contains(s []string, elem string) bool {
	for _, v := range s {
		if v == elem {
			return true
		}
	}
	return false
}

// ----------------------------------------------------------------
// TestDetectWorkTypesPavement
// Ports: test_detect_work_types_pavement
// 舗装工 is detected when photoCategory="到着温度" scene="アスファルト舗装"
// ----------------------------------------------------------------
func TestDetectWorkTypesPavement(t *testing.T) {
	rawDataSlice := []ai.RawImageData{
		rawData("temp1.jpg", "到着温度", "", "アスファルト舗装"),
	}
	types := DetectWorkTypes(rawDataSlice)
	if !contains(types, "舗装工") {
		t.Errorf("expected 舗装工 in result, got %v", types)
	}
}

// ----------------------------------------------------------------
// TestDetectWorkTypesMarking
// Ports: test_detect_work_types_marking
// 区画線工 is detected from detectedText="区画線施工" scene="白線を引いている"
// ----------------------------------------------------------------
func TestDetectWorkTypesMarking(t *testing.T) {
	rawDataSlice := []ai.RawImageData{
		rawData("line1.jpg", "", "区画線施工", "白線を引いている"),
	}
	types := DetectWorkTypes(rawDataSlice)
	if !contains(types, "区画線工") {
		t.Errorf("expected 区画線工 in result, got %v", types)
	}
}

// ----------------------------------------------------------------
// TestDetectWorkTypesMultiple
// Ports: test_detect_work_types_multiple
// Three images → three distinct work types, len==3
// ----------------------------------------------------------------
func TestDetectWorkTypesMultiple(t *testing.T) {
	rawDataSlice := []ai.RawImageData{
		rawData("temp1.jpg", "転圧状況", "", "ローラーで転圧"),
		rawData("line1.jpg", "", "", "区画線の白線"),
		rawData("demolish1.jpg", "取壊し状況", "", "解体作業"),
	}
	types := DetectWorkTypes(rawDataSlice)
	if !contains(types, "舗装工") {
		t.Errorf("expected 舗装工 in result, got %v", types)
	}
	if !contains(types, "区画線工") {
		t.Errorf("expected 区画線工 in result, got %v", types)
	}
	if !contains(types, "構造物撤去工") {
		t.Errorf("expected 構造物撤去工 in result, got %v", types)
	}
	if len(types) != 3 {
		t.Errorf("expected exactly 3 work types, got %d: %v", len(types), types)
	}
}

// ----------------------------------------------------------------
// TestDetectWorkTypesEmpty
// Ports: test_detect_work_types_empty
// No keywords match → empty result
// ----------------------------------------------------------------
func TestDetectWorkTypesEmpty(t *testing.T) {
	rawDataSlice := []ai.RawImageData{
		rawData("other.jpg", "その他", "", "風景写真"),
	}
	types := DetectWorkTypes(rawDataSlice)
	if len(types) != 0 {
		t.Errorf("expected empty result, got %v", types)
	}
}

// ----------------------------------------------------------------
// TestDetectWorkTypesDrainage
// Ports: test_detect_work_types_drainage
// 排水構造物工 via detectedText="側溝設置" scene="マンホール"
// ----------------------------------------------------------------
func TestDetectWorkTypesDrainage(t *testing.T) {
	rawDataSlice := []ai.RawImageData{
		rawData("drain1.jpg", "", "側溝設置", "マンホール"),
	}
	types := DetectWorkTypes(rawDataSlice)
	if !contains(types, "排水構造物工") {
		t.Errorf("expected 排水構造物工 in result, got %v", types)
	}
}

// ----------------------------------------------------------------
// TestDetectWorkTypesManhole
// Ports: test_detect_work_types_manhole
// 人孔改良工 via detectedText="人孔改良工事"
// ----------------------------------------------------------------
func TestDetectWorkTypesManhole(t *testing.T) {
	rawDataSlice := []ai.RawImageData{
		rawData("manhole1.jpg", "", "人孔改良工事", ""),
	}
	types := DetectWorkTypes(rawDataSlice)
	if !contains(types, "人孔改良工") {
		t.Errorf("expected 人孔改良工 in result, got %v", types)
	}
}

// ----------------------------------------------------------------
// TestDetectWorkTypesEarthwork
// Ports: test_detect_work_types_earthwork
// 道路土工 via photoCategory="掘削状況" scene="バックホウで掘削"
// ----------------------------------------------------------------
func TestDetectWorkTypesEarthwork(t *testing.T) {
	rawDataSlice := []ai.RawImageData{
		rawData("earth1.jpg", "掘削状況", "", "バックホウで掘削"),
	}
	types := DetectWorkTypes(rawDataSlice)
	if !contains(types, "道路土工") {
		t.Errorf("expected 道路土工 in result, got %v", types)
	}
}

// ----------------------------------------------------------------
// TestDetectWorkTypesWithCustomDefs
// Smoke test for DetectWorkTypesWith with caller-supplied definitions.
// Not in Rust tests, but validates the exported API.
// ----------------------------------------------------------------
func TestDetectWorkTypesWithCustomDefs(t *testing.T) {
	defs := []WorkTypeDefinition{
		{
			Name:          "ダミー工種",
			SceneKeywords: []string{"ダミー"},
		},
	}
	rawDataSlice := []ai.RawImageData{
		rawData("dummy.jpg", "", "", "ダミー施工現場"),
	}
	types := DetectWorkTypesWith(rawDataSlice, defs)
	if !contains(types, "ダミー工種") {
		t.Errorf("expected ダミー工種 in result, got %v", types)
	}
}

// ----------------------------------------------------------------
// TestResponseParseError
// Validates AnalyzerError::ResponseParse Go mirror.
// ----------------------------------------------------------------
func TestResponseParseError(t *testing.T) {
	err := NewResponseParseError("invalid JSON near offset 42")
	if err == nil {
		t.Fatal("expected non-nil error")
	}
	msg := err.Error()
	if !strings.Contains(msg, "AI response parse error") {
		t.Errorf("unexpected error message: %s", msg)
	}
}

// ----------------------------------------------------------------
// TestErrTaggerEmpty
// Validates AnalyzerError::TaggerEmpty Go mirror.
// ----------------------------------------------------------------
func TestErrTaggerEmpty(t *testing.T) {
	if ErrTaggerEmpty == nil {
		t.Fatal("ErrTaggerEmpty must not be nil")
	}
	if ErrTaggerEmpty.Error() == "" {
		t.Fatal("ErrTaggerEmpty message must not be empty")
	}
}
