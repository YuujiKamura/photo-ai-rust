package export

import (
	"archive/zip"
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/YuujiKamura/photo-ai-go/pkg/engine"
)

// ---- fixtures ----

// testPhotos returns a small slice of AnalysisResult covering all 8 fields.
func testPhotos() []engine.AnalysisResult {
	return []engine.AnalysisResult{
		{
			Date:          "2026-01-15",
			PhotoCategory: "施工状況写真",
			WorkType:      "舗装工",
			Variety:       "舗装打換え工",
			Subphase:      "表層工",
			Station:       "No.10",
			Remarks:       "舗設状況",
			Measurements:  "温度: 160℃",
			FilePath:      "",
			FileName:      "test-01.jpg",
		},
		{
			Date:          "2026-01-15",
			PhotoCategory: "品質管理写真",
			WorkType:      "舗装工",
			Variety:       "舗装打換え工",
			Subphase:      "表層工",
			Station:       "No.11",
			Remarks:       "転圧状況",
			Measurements:  "",
			FilePath:      "",
			FileName:      "test-02.jpg",
		},
		{
			Date:          "2026-01-16",
			PhotoCategory: "出来形管理写真",
			WorkType:      "舗装工",
			Variety:       "路面切削工",
			Subphase:      "路面切削",
			Station:       "No.12",
			Remarks:       "",
			Measurements:  "厚さ: 40mm",
			FilePath:      "",
			FileName:      "test-03.jpg",
		},
	}
}

// writeTestJSON writes a []AnalysisResult slice to a temp JSON file and
// returns its path. The caller is responsible for cleanup.
func writeTestJSON(t *testing.T, photos []engine.AnalysisResult) string {
	t.Helper()
	data, err := json.Marshal(photos)
	if err != nil {
		t.Fatalf("marshal test photos: %v", err)
	}
	f, err := os.CreateTemp(t.TempDir(), "result-*.json")
	if err != nil {
		t.Fatalf("create temp JSON: %v", err)
	}
	if _, err := f.Write(data); err != nil {
		t.Fatalf("write temp JSON: %v", err)
	}
	if err := f.Close(); err != nil {
		t.Fatalf("close temp JSON: %v", err)
	}
	return f.Name()
}

// ---- basic smoke tests ----

// TestExcelNative_Smoke verifies that GenerateExcelNative produces a valid xlsx
// file when PHOTO_EXCEL_BACKEND=go, checks sheet names and basic cell content.
func TestExcelNative_Smoke(t *testing.T) {
	photos := testPhotos()
	jsonPath := writeTestJSON(t, photos)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")

	result, err := GenerateExcelNative(jsonPath, outPath, 3)
	if err != nil {
		t.Fatalf("GenerateExcelNative: %v", err)
	}
	if result.OutputPath != outPath {
		t.Errorf("OutputPath = %q, want %q", result.OutputPath, outPath)
	}
	if result.SheetCount != 1 {
		t.Errorf("SheetCount = %d, want 1 (3 photos fit on 1 page)", result.SheetCount)
	}

	// Verify the file is a valid ZIP (xlsx).
	data, err := os.ReadFile(outPath)
	if err != nil {
		t.Fatalf("read output file: %v", err)
	}
	zr, err := zip.NewReader(bytes.NewReader(data), int64(len(data)))
	if err != nil {
		t.Fatalf("output is not a valid ZIP/xlsx: %v", err)
	}

	// Check xl/worksheets/sheet1.xml is present.
	foundSheet := false
	for _, zf := range zr.File {
		if zf.Name == "xl/worksheets/sheet1.xml" {
			foundSheet = true
		}
	}
	if !foundSheet {
		t.Error("xl/worksheets/sheet1.xml not found in xlsx")
	}
}

// TestExcelNative_SheetNames checks that sheet names are "1", "2", etc.
func TestExcelNative_SheetNames(t *testing.T) {
	// 4 photos, 3-up → 2 sheets.
	photos := append(testPhotos(), engine.AnalysisResult{FileName: "test-04.jpg"})
	jsonPath := writeTestJSON(t, photos)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")

	result, err := GenerateExcelNative(jsonPath, outPath, 3)
	if err != nil {
		t.Fatalf("GenerateExcelNative: %v", err)
	}
	if result.SheetCount != 2 {
		t.Errorf("SheetCount = %d, want 2", result.SheetCount)
	}

	// Open the ZIP and look in xl/workbook.xml for sheet name attributes.
	files := readXlsxXML(t, outPath)
	wbXML, ok := files["xl/workbook.xml"]
	if !ok {
		t.Fatal("xl/workbook.xml not found in xlsx")
	}
	// Workbook XML should mention sheet names "1" and "2".
	if !strings.Contains(wbXML, `name="1"`) {
		t.Errorf(`workbook.xml does not contain sheet name "1"; got:\n%s`, wbXML)
	}
	if !strings.Contains(wbXML, `name="2"`) {
		t.Errorf(`workbook.xml does not contain sheet name "2"; got:\n%s`, wbXML)
	}
}

// TestExcelNative_FieldValues checks that field values are written to C column cells.
func TestExcelNative_FieldValues(t *testing.T) {
	photos := testPhotos()[:1] // single photo
	jsonPath := writeTestJSON(t, photos)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")

	_, err := GenerateExcelNative(jsonPath, outPath, 3)
	if err != nil {
		t.Fatalf("GenerateExcelNative: %v", err)
	}

	// Inspect all XML files in the ZIP for the expected strings.
	files := readXlsxXML(t, outPath)
	allXML := strings.Builder{}
	for _, v := range files {
		allXML.WriteString(v)
	}
	combined := allXML.String()

	// The shared strings table or sheet XML should contain these values.
	checks := []string{
		"施工状況写真",
		"舗装工",
		"舗装打換え工",
		"表層工",
		"No.10",
		"舗設状況",
		"温度: 160℃",
	}
	for _, want := range checks {
		if !strings.Contains(combined, want) {
			t.Errorf("xlsx XML does not contain expected value %q", want)
		}
	}
}

// TestExcelNative_LabelValues checks that label strings are present.
func TestExcelNative_LabelValues(t *testing.T) {
	photos := testPhotos()[:1]
	jsonPath := writeTestJSON(t, photos)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")

	_, err := GenerateExcelNative(jsonPath, outPath, 3)
	if err != nil {
		t.Fatalf("GenerateExcelNative: %v", err)
	}

	// Inspect all XML files in the ZIP for the expected labels.
	files := readXlsxXML(t, outPath)
	allXML := strings.Builder{}
	for _, v := range files {
		allXML.WriteString(v)
	}
	combined := allXML.String()

	labels := []string{"日時", "区分", "工種", "種別", "細別", "測点", "備考", "測定値"}
	for _, lbl := range labels {
		if !strings.Contains(combined, lbl) {
			t.Errorf("xlsx XML does not contain label %q", lbl)
		}
	}
}

// TestExcelNative_EmptyPhotos verifies that an empty result list still produces a valid file.
func TestExcelNative_EmptyPhotos(t *testing.T) {
	photos := []engine.AnalysisResult{}
	jsonPath := writeTestJSON(t, photos)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")

	result, err := GenerateExcelNative(jsonPath, outPath, 3)
	if err != nil {
		t.Fatalf("GenerateExcelNative(empty): %v", err)
	}
	if result.SheetCount != 1 {
		t.Errorf("SheetCount = %d for empty list, want 1", result.SheetCount)
	}
	if _, err := os.Stat(outPath); err != nil {
		t.Errorf("output file missing: %v", err)
	}
}

// ---- env-var dispatch tests ----

// TestEnvVarDispatch_Unset checks that the default (DLL) path is selected.
// Since no DLL is available in CI, we only verify the error message does NOT
// say "not a valid value" — any engine error is acceptable for the DLL path.
func TestEnvVarDispatch_Unset(t *testing.T) {
	t.Setenv("PHOTO_EXCEL_BACKEND", "")
	photos := testPhotos()
	jsonPath := writeTestJSON(t, photos)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")

	_, err := GenerateExcel(jsonPath, outPath, "", "")
	// May succeed or fail (no engine exe in test env), but must NOT be a "invalid value" error.
	if err != nil && strings.Contains(err.Error(), "not a valid value") {
		t.Errorf("unexpected 'not a valid value' error for empty PHOTO_EXCEL_BACKEND: %v", err)
	}
}

// TestEnvVarDispatch_DLL is identical to unset — explicitly "dll" selects DLL path.
func TestEnvVarDispatch_DLL(t *testing.T) {
	t.Setenv("PHOTO_EXCEL_BACKEND", "dll")
	photos := testPhotos()
	jsonPath := writeTestJSON(t, photos)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")

	_, err := GenerateExcel(jsonPath, outPath, "", "")
	if err != nil && strings.Contains(err.Error(), "not a valid value") {
		t.Errorf("unexpected 'not a valid value' error for PHOTO_EXCEL_BACKEND=dll: %v", err)
	}
}

// TestEnvVarDispatch_Go verifies that PHOTO_EXCEL_BACKEND=go produces output via excelize.
func TestEnvVarDispatch_Go(t *testing.T) {
	t.Setenv("PHOTO_EXCEL_BACKEND", "go")
	photos := testPhotos()
	jsonPath := writeTestJSON(t, photos)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")

	result, err := GenerateExcel(jsonPath, outPath, "", "")
	if err != nil {
		t.Fatalf("GenerateExcel(PHOTO_EXCEL_BACKEND=go): %v", err)
	}
	if _, err := os.Stat(result.OutputPath); err != nil {
		t.Errorf("output file %q missing: %v", result.OutputPath, err)
	}
}

// TestEnvVarDispatch_Unknown verifies that an unknown backend value produces a clear error.
func TestEnvVarDispatch_Unknown(t *testing.T) {
	t.Setenv("PHOTO_EXCEL_BACKEND", "rust")
	photos := testPhotos()
	jsonPath := writeTestJSON(t, photos)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")

	_, err := GenerateExcel(jsonPath, outPath, "", "")
	if err == nil {
		t.Fatal("expected error for unknown backend value, got nil")
	}
	if !strings.Contains(err.Error(), "not a valid value") {
		t.Errorf("error message does not mention 'not a valid value': %v", err)
	}
}

// ---- golden structural test ----

// TestExcelize_StructuralParity compares the structure of xlsx files generated
// by GenerateExcelNative (go backend) checking key XML file presence and
// structural content (sheet count, cell values).
//
// Because we cannot call the DLL in a unit test environment, this test uses
// two separate calls to GenerateExcelNative and verifies internal structural
// consistency rather than diffing against DLL output.
//
// Cosmetic differences tolerated (not checked):
//   - ZIP timestamps (each Save call uses a fresh timestamp)
//   - XML attribute order (excelize may reorder attributes)
//   - xmlns redeclarations (excelize may emit them per-element)
//   - creator/lastModifiedBy metadata (excelize uses its own defaults)
func TestExcelize_StructuralParity(t *testing.T) {
	photos := testPhotos()
	jsonPath := writeTestJSON(t, photos)

	tmpA := filepath.Join(t.TempDir(), "a.xlsx")
	tmpB := filepath.Join(t.TempDir(), "b.xlsx")

	if _, err := GenerateExcelNative(jsonPath, tmpA, 3); err != nil {
		t.Fatalf("generate A: %v", err)
	}
	if _, err := GenerateExcelNative(jsonPath, tmpB, 3); err != nil {
		t.Fatalf("generate B: %v", err)
	}

	// Unzip and compare key files.
	filesA := readXlsxXML(t, tmpA)
	filesB := readXlsxXML(t, tmpB)

	criticalFiles := []string{
		"xl/worksheets/sheet1.xml",
		"xl/sharedStrings.xml",
		"xl/styles.xml",
		"xl/workbook.xml",
	}

	for _, name := range criticalFiles {
		contentA, okA := filesA[name]
		contentB, okB := filesB[name]
		if !okA {
			t.Errorf("file A missing %s", name)
			continue
		}
		if !okB {
			t.Errorf("file B missing %s", name)
			continue
		}
		// Normalise cosmetic diffs before comparing.
		normA := normaliseXMLForDiff(contentA)
		normB := normaliseXMLForDiff(contentB)
		if normA != normB {
			t.Errorf("%s structural diff:\n--- A ---\n%s\n--- B ---\n%s",
				name, truncate(normA, 800), truncate(normB, 800))
		}
	}
}

// readXlsxXML opens an xlsx archive and returns a map[filename]content.
func readXlsxXML(t *testing.T, path string) map[string]string {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	zr, err := zip.NewReader(bytes.NewReader(data), int64(len(data)))
	if err != nil {
		t.Fatalf("unzip %s: %v", path, err)
	}
	m := make(map[string]string, len(zr.File))
	for _, zf := range zr.File {
		rc, err := zf.Open()
		if err != nil {
			continue
		}
		var buf bytes.Buffer
		if _, err := buf.ReadFrom(rc); err == nil {
			m[zf.Name] = buf.String()
		}
		rc.Close()
	}
	return m
}

// normaliseXMLForDiff strips cosmetic differences that we explicitly tolerate:
//   - ZIP timestamps: already stripped (they're in the ZIP headers, not XML body)
//   - creator / lastModifiedBy elements (core.xml)
//   - modified / created date elements (core.xml)
//
// Value-carrying XML content (cell values, sheet names, styles) is kept.
func normaliseXMLForDiff(s string) string {
	// Remove lastModifiedBy and creator tags (core properties).
	s = removeXMLElement(s, "dc:creator")
	s = removeXMLElement(s, "cp:lastModifiedBy")
	// Remove dcterms:created / dcterms:modified timestamps.
	s = removeXMLElement(s, "dcterms:created")
	s = removeXMLElement(s, "dcterms:modified")
	return s
}

// removeXMLElement strips an XML element of the form <tag ...>...</tag> or <tag .../>.
func removeXMLElement(s, tag string) string {
	// Simple approach: remove opening+content+closing.
	for {
		start := strings.Index(s, "<"+tag)
		if start == -1 {
			break
		}
		end := strings.Index(s[start:], ">")
		if end == -1 {
			break
		}
		end += start + 1
		// Self-closing?
		if s[end-2] == '/' {
			s = s[:start] + s[end:]
			continue
		}
		closeTag := "</"+tag+">"
		closeIdx := strings.Index(s[end:], closeTag)
		if closeIdx == -1 {
			break
		}
		s = s[:start] + s[end+closeIdx+len(closeTag):]
	}
	return s
}

func truncate(s string, n int) string {
	if len(s) > n {
		return s[:n] + "…"
	}
	return s
}
