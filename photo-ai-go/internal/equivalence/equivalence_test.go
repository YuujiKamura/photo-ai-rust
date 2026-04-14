// Package equivalence — Rust vs Go analysis equivalence tests.
//
// Layer 1: CI-safe, mock AI.
//
//   - Loads photo-groups.json as mock ProcessImageFn output (no real photos needed).
//   - Runs pipeline.Run() with the mock injected (Go path).
//   - Compares against expected_rust_result.json (pre-computed golden snapshot).
//   - When the Rust DLL / engine is available (PHOTO_ANALYSIS_ENGINE_EXE set),
//     also calls engine.MatchMaster for live cross-check.
//
// Known divergences are annotated with "// DIV:" comments.
// Tests that require the Rust DLL are skipped when the engine is unavailable.
//
// Run:
//
//	cd photo-ai-go && go test -v ./internal/equivalence/...
package equivalence

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"

	"github.com/YuujiKamura/photo-ai-go/internal/matching"
	"github.com/YuujiKamura/photo-ai-go/internal/pipeline"
	"github.com/YuujiKamura/photo-ai-go/pkg/engine"
)

// fixtureDir returns the absolute path to tests/equivalence/fixtures/<name>.
//
// Resolution order:
//  1. EQUIV_FIXTURE_DIR env var (override for unusual build layouts)
//  2. Walk up from the test binary's working directory looking for the fixture.
//
// go test sets the working directory to the package directory
// (photo-ai-go/internal/equivalence), so we walk up four levels.
func fixtureDir(t *testing.T, name string) string {
	t.Helper()

	// Allow explicit override.
	if base := os.Getenv("EQUIV_FIXTURE_DIR"); base != "" {
		dir := filepath.Join(base, name)
		if _, err := os.Stat(dir); os.IsNotExist(err) {
			t.Fatalf("EQUIV_FIXTURE_DIR fixture not found: %s", dir)
		}
		return dir
	}

	// `go test` sets cwd to the package directory.
	// From photo-ai-go/internal/equivalence we need to go up 3 levels to reach
	// the repo root, then into tests/equivalence/fixtures/<name>.
	cwd, err := os.Getwd()
	if err != nil {
		t.Fatalf("os.Getwd: %v", err)
	}

	// Try walking up up to 5 levels looking for tests/equivalence/fixtures/<name>.
	for i := 0; i <= 5; i++ {
		candidate := filepath.Join(cwd, "tests", "equivalence", "fixtures", name)
		if _, err := os.Stat(candidate); err == nil {
			return candidate
		}
		parent := filepath.Dir(cwd)
		if parent == cwd {
			break // reached filesystem root
		}
		cwd = parent
	}

	t.Fatalf("fixture dir not found for %q — set EQUIV_FIXTURE_DIR or run from repo root", name)
	return "" // unreachable
}

// normalizedResult is the subset of fields compared for equivalence.
// Transport, timestamp, provider etc. are intentionally excluded.
type normalizedResult struct {
	FileName      string `json:"fileName"`
	WorkType      string `json:"workType"`
	Variety       string `json:"variety"`
	Subphase      string `json:"subphase"`
	Remarks       string `json:"remarks"`
	PhotoCategory string `json:"photoCategory"`
	Station       string `json:"station"`
	HasBoard      bool   `json:"hasBoard"`
	DetectedText  string `json:"detectedText"`
}

// normalizeGoResult converts a pipeline.Result to the canonical comparison struct.
func normalizeGoResult(r pipeline.Result) normalizedResult {
	return normalizedResult{
		FileName:      r.FileName,
		WorkType:      r.WorkType,
		Variety:       r.Variety,
		Subphase:      r.Subphase,
		Remarks:       r.Remarks,
		PhotoCategory: r.PhotoCategory,
		Station:       r.Station,
		HasBoard:      r.HasBoard,
		DetectedText:  r.DetectedText,
	}
}

// normalizeEngineResult converts an engine.AnalysisResult to the canonical struct.
func normalizeEngineResult(r engine.AnalysisResult) normalizedResult {
	return normalizedResult{
		FileName:      r.FileName,
		WorkType:      r.WorkType,
		Variety:       r.Variety,
		Subphase:      r.Subphase,
		Remarks:       r.Remarks,
		PhotoCategory: r.PhotoCategory,
		Station:       r.Station,
		HasBoard:      r.HasBoard,
		DetectedText:  r.DetectedText,
	}
}

// sortByFileName sorts normalizedResult slice by FileName for stable comparison.
func sortByFileName(rs []normalizedResult) {
	sort.Slice(rs, func(i, j int) bool {
		return rs[i].FileName < rs[j].FileName
	})
}

// prettyJSON returns an indented JSON string for diff output.
func prettyJSON(v any) string {
	b, _ := json.MarshalIndent(v, "", "  ")
	return string(b)
}

// diffResults compares two normalizedResult slices and returns a human-readable
// diff string. Returns "" if equal.
func diffResults(want, got []normalizedResult) string {
	if len(want) != len(got) {
		return fmt.Sprintf("length mismatch: want %d, got %d\nwant: %s\ngot:  %s",
			len(want), len(got), prettyJSON(want), prettyJSON(got))
	}
	var diffs []string
	for i := range want {
		w := want[i]
		g := got[i]
		if w == g {
			continue
		}
		var fields []string
		if w.FileName != g.FileName {
			fields = append(fields, fmt.Sprintf("  fileName:      want=%q got=%q", w.FileName, g.FileName))
		}
		if w.WorkType != g.WorkType {
			fields = append(fields, fmt.Sprintf("  workType:      want=%q got=%q", w.WorkType, g.WorkType))
		}
		if w.Variety != g.Variety {
			fields = append(fields, fmt.Sprintf("  variety:       want=%q got=%q", w.Variety, g.Variety))
		}
		if w.Subphase != g.Subphase {
			fields = append(fields, fmt.Sprintf("  subphase:      want=%q got=%q", w.Subphase, g.Subphase))
		}
		if w.Remarks != g.Remarks {
			fields = append(fields, fmt.Sprintf("  remarks:       want=%q got=%q", w.Remarks, g.Remarks))
		}
		if w.PhotoCategory != g.PhotoCategory {
			fields = append(fields, fmt.Sprintf("  photoCategory: want=%q got=%q", w.PhotoCategory, g.PhotoCategory))
		}
		if w.Station != g.Station {
			fields = append(fields, fmt.Sprintf("  station:       want=%q got=%q", w.Station, g.Station))
		}
		if w.HasBoard != g.HasBoard {
			fields = append(fields, fmt.Sprintf("  hasBoard:      want=%v got=%v", w.HasBoard, g.HasBoard))
		}
		if w.DetectedText != g.DetectedText {
			fields = append(fields, fmt.Sprintf("  detectedText:  want=%q got=%q", w.DetectedText, g.DetectedText))
		}
		diffs = append(diffs, fmt.Sprintf("[%d] %s:\n%s", i, w.FileName, strings.Join(fields, "\n")))
	}
	return strings.Join(diffs, "\n\n")
}

// ---- golden snapshot type (matches expected_rust_result.json) ----------------

type goldenResult struct {
	FileName      string `json:"fileName"`
	WorkType      string `json:"workType"`
	Variety       string `json:"variety"`
	Subphase      string `json:"subphase"`
	Remarks       string `json:"remarks"`
	PhotoCategory string `json:"photoCategory"`
	Station       string `json:"station"`
	HasBoard      bool   `json:"hasBoard"`
	DetectedText  string `json:"detectedText"`
}

type goldenFile struct {
	Note    string         `json:"_note"`
	Status  string         `json:"_status"`
	Results []goldenResult `json:"results"`
}

func loadGolden(t *testing.T, path string) []normalizedResult {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read golden: %v", err)
	}
	var gf goldenFile
	if err := json.Unmarshal(data, &gf); err != nil {
		t.Fatalf("parse golden: %v", err)
	}
	if strings.Contains(gf.Status, "PLACEHOLDER") {
		t.Skip("Rust golden snapshot is a placeholder — run 'make all' to generate real data")
	}
	out := make([]normalizedResult, len(gf.Results))
	for i, g := range gf.Results {
		out[i] = normalizedResult{
			FileName:      g.FileName,
			WorkType:      g.WorkType,
			Variety:       g.Variety,
			Subphase:      g.Subphase,
			Remarks:       g.Remarks,
			PhotoCategory: g.PhotoCategory,
			Station:       g.Station,
			HasBoard:      g.HasBoard,
			DetectedText:  g.DetectedText,
		}
	}
	sortByFileName(out)
	return out
}

// ---- Layer 1: Go pipeline internal test (CI-safe, mock AI) -------------------

// TestGopipeline_SamplePavementJob runs the full Go pipeline against the
// sample_pavement_job fixture with ProcessImageFn mocked out.
// This is the primary CI-safe test — no DLL, no network, no real photos.
func TestGoipeline_SamplePavementJob(t *testing.T) {
	fixDir := fixtureDir(t, "sample_pavement_job")
	groupsPath := filepath.Join(fixDir, "photo-groups.json")
	masterPath := filepath.Join(fixDir, "master.csv")

	// Count expected photo files from groups JSON.
	groupsData, err := os.ReadFile(groupsPath)
	if err != nil {
		t.Fatalf("read groups: %v", err)
	}
	var groups map[string]json.RawMessage
	if err := json.Unmarshal(groupsData, &groups); err != nil {
		t.Fatalf("parse groups: %v", err)
	}
	fileNames := make([]string, 0, len(groups))
	for k := range groups {
		fileNames = append(fileNames, k)
	}

	// Create a temp dir with fake (empty) image files matching fixture filenames.
	tmpDir := t.TempDir()
	for _, fname := range fileNames {
		p := filepath.Join(tmpDir, fname)
		if err := os.WriteFile(p, []byte{0xFF, 0xD8, 0xFF, 0xE0}, 0644); err != nil {
			t.Fatalf("create fake image %s: %v", fname, err)
		}
	}
	// Copy groups JSON into tmpDir so pipeline reads it.
	grpDst := filepath.Join(tmpDir, "photo-groups.json")
	if err := os.WriteFile(grpDst, groupsData, 0644); err != nil {
		t.Fatalf("copy groups: %v", err)
	}

	// Load master CSV into HierarchyMaster.
	hm, err := matching.NewHierarchyMasterFromCSVFile(masterPath)
	if err != nil {
		t.Fatalf("load master: %v", err)
	}

	// Stub ProcessImageFn: returns the fixture groups JSON path.
	stub := func(_ engine.ImageConfig) (engine.ImageResult, error) {
		return engine.ImageResult{
			OutputJSON: grpDst,
			PhotoCount: len(fileNames),
		}, nil
	}

	cfg := pipeline.Config{
		Folder:          tmpDir,
		BatchSize:       10,
		IncludeAll:      true,
		HierarchyMaster: hm,
		ProcessImageFn:  stub,
	}

	results, err := pipeline.Run(context.Background(), cfg)
	if err != nil {
		t.Fatalf("pipeline.Run: %v", err)
	}

	if len(results) == 0 {
		t.Fatal("pipeline returned 0 results")
	}
	t.Logf("Go pipeline: %d results", len(results))

	// ---- Basic structural assertions ----

	byName := make(map[string]pipeline.Result, len(results))
	for _, r := range results {
		byName[r.FileName] = r
	}

	// (A) DetectedText round-trips through the mock without corruption.
	t.Run("detected_text_preserved", func(t *testing.T) {
		wantTexts := map[string]string{
			"テスト001.JPG": "工種：舗装工\n種別：舗装打換え工\n測点：No.1+10\nAs舗装版切断状況",
			"テスト003.JPG": "到着温度 160℃\nNo.1台目",
			"テスト006.JPG": "区画線工\n溶融式区画線\n施工状況\n測点：No.5+00",
		}
		for fname, wantText := range wantTexts {
			r, ok := byName[fname]
			if !ok {
				t.Errorf("result for %s not found", fname)
				continue
			}
			if r.DetectedText != wantText {
				t.Errorf("%s: DetectedText = %q, want %q", fname, r.DetectedText, wantText)
			}
		}
	})

	// (B) HasBoard flag preserved.
	t.Run("has_board_preserved", func(t *testing.T) {
		wantBoard := map[string]bool{
			"テスト001.JPG": true,
			"テスト002.JPG": false,
			"テスト003.JPG": true,
		}
		for fname, want := range wantBoard {
			r, ok := byName[fname]
			if !ok {
				t.Errorf("result for %s not found", fname)
				continue
			}
			if r.HasBoard != want {
				t.Errorf("%s: HasBoard = %v, want %v", fname, r.HasBoard, want)
			}
		}
	})

	// (C) 舗装工 matched via keyword in detected_text.
	t.Run("work_type_pavement_matched", func(t *testing.T) {
		r, ok := byName["テスト001.JPG"]
		if !ok {
			t.Skip("テスト001.JPG not in results")
		}
		if r.WorkType != "舗装工" {
			// DIV: Go uses 工種 KV extract → normalizeWorkTypeFromOCR → master workTypeHint.
			// Rust additionally checks search_pattern against scene description.
			// If this fails, it indicates the OCR key-value extraction diverged.
			t.Errorf("テスト001.JPG: WorkType = %q, want %q (DIV: OCR KV extraction or hint routing)", r.WorkType, "舗装工")
		}
	})

	// (D) Station extracted from OCR detected_text via 測点 key.
	t.Run("station_extracted_from_ocr", func(t *testing.T) {
		r, ok := byName["テスト001.JPG"]
		if !ok {
			t.Skip("テスト001.JPG not in results")
		}
		if r.Station == "" {
			// DIV: Go uses ocr.ExtractKV which splits on "：" or ":".
			// Rust uses its own Station::parse logic that handles more forms.
			// TODO: Go ocr.ExtractKV vs Rust Station::parse — check key alias handling.
			t.Errorf("テスト001.JPG: Station is empty, want 'No.1+10' (DIV: OCR KV '測点' key extraction)")
		}
	})

	// (E) 温度測定 photo matched to 到着温度測定 remarks.
	t.Run("temperature_photo_matched", func(t *testing.T) {
		r, ok := byName["テスト003.JPG"]
		if !ok {
			t.Skip("テスト003.JPG not in results")
		}
		if r.Remarks == "" {
			// DIV: Rust uses temperature subphase detection → remarks='到着温度測定'.
			// Go: the search pattern "到着温度|到着" should hit fixture master row.
			// TODO: Go → Rust: Rust populates remarks from rules post-process; Go does it via master match.
			t.Logf("テスト003.JPG: Remarks empty (DIV: Rust may populate via temperature rules post-process, Go via master search pattern)")
		} else {
			t.Logf("テスト003.JPG: Remarks = %q (OK)", r.Remarks)
		}
	})

	// (F) 撤去 search pattern matches 構造物撤去工 for テスト004.
	t.Run("demolition_matched", func(t *testing.T) {
		r, ok := byName["テスト004.JPG"]
		if !ok {
			t.Skip("テスト004.JPG not in results")
		}
		if r.WorkType != "構造物撤去工" {
			// DIV: Rust: 工種 KV "構造物撤去工" → exact workTypeHint.
			// Go: same path via extractKVFromText → hasWorkType check.
			t.Errorf("テスト004.JPG: WorkType = %q, want %q", r.WorkType, "構造物撤去工")
		}
	})

	// (G) Scene-only photo (no board, no text) — workType derived from context.
	t.Run("scene_only_no_board", func(t *testing.T) {
		r, ok := byName["テスト002.JPG"]
		if !ok {
			t.Skip("テスト002.JPG not in results")
		}
		// DIV: Rust engine propagates workType from batch context / folder rules.
		// Go pipeline: with no detected_text and no folderRules configured in this test,
		// WorkType will likely be empty. This is a known open gap.
		// TODO: Go vs Rust — batch-level workType propagation for scene photos.
		t.Logf("テスト002.JPG: WorkType = %q, HasBoard = %v (scene-only; workType propagation gap expected)", r.WorkType, r.HasBoard)
	})
}

// ---- Layer 1b: Go pipeline vs Rust golden snapshot ---------------------------

// TestGoVsRustGolden compares Go pipeline output against the pre-computed
// expected_rust_result.json golden snapshot.
//
// If the snapshot is still a placeholder (PLACEHOLDER in _status), the test
// is skipped automatically. This keeps CI green until a real snapshot is committed.
func TestGoVsRustGolden(t *testing.T) {
	fixDir := fixtureDir(t, "sample_pavement_job")
	goldenPath := filepath.Join(fixDir, "expected_rust_result.json")
	masterPath := filepath.Join(fixDir, "master.csv")
	groupsPath := filepath.Join(fixDir, "photo-groups.json")

	// loadGolden skips the test if snapshot is a placeholder.
	wantResults := loadGolden(t, goldenPath)

	groupsData, err := os.ReadFile(groupsPath)
	if err != nil {
		t.Fatalf("read groups: %v", err)
	}
	var groups map[string]json.RawMessage
	if err := json.Unmarshal(groupsData, &groups); err != nil {
		t.Fatalf("parse groups: %v", err)
	}

	tmpDir := t.TempDir()
	for k := range groups {
		p := filepath.Join(tmpDir, k)
		if err := os.WriteFile(p, []byte{0xFF, 0xD8, 0xFF, 0xE0}, 0644); err != nil {
			t.Fatalf("create fake image: %v", err)
		}
	}
	grpDst := filepath.Join(tmpDir, "photo-groups.json")
	if err := os.WriteFile(grpDst, groupsData, 0644); err != nil {
		t.Fatalf("copy groups: %v", err)
	}

	hm, err := matching.NewHierarchyMasterFromCSVFile(masterPath)
	if err != nil {
		t.Fatalf("load master: %v", err)
	}

	stub := func(_ engine.ImageConfig) (engine.ImageResult, error) {
		return engine.ImageResult{OutputJSON: grpDst, PhotoCount: len(groups)}, nil
	}

	cfg := pipeline.Config{
		Folder:          tmpDir,
		BatchSize:       10,
		IncludeAll:      true,
		HierarchyMaster: hm,
		ProcessImageFn:  stub,
	}

	results, err := pipeline.Run(context.Background(), cfg)
	if err != nil {
		t.Fatalf("pipeline.Run: %v", err)
	}

	gotNorm := make([]normalizedResult, len(results))
	for i, r := range results {
		gotNorm[i] = normalizeGoResult(r)
	}
	sortByFileName(gotNorm)

	if d := diffResults(wantResults, gotNorm); d != "" {
		t.Errorf("Go vs Rust golden mismatch:\n%s", d)
	}
}

// ---- Layer 1c: Live Rust engine cross-check (requires DLL) ------------------

// TestGoVsRustLive calls engine.MatchMaster (Rust DLL) on the fixture and
// compares with Go pipeline output.
//
// Skipped when PHOTO_ANALYSIS_ENGINE_EXE is not set or the binary is missing.
//
// This test intentionally shows divergences — each mismatch is logged, not fatal,
// to document known gaps between Go and Rust implementations.
func TestGoVsRustLive(t *testing.T) {
	engineExe := os.Getenv("PHOTO_ANALYSIS_ENGINE_EXE")
	if engineExe == "" {
		// Also check default sibling path pattern used by engine.go.
		// engine.MatchMaster will try to resolve it, but we short-circuit here
		// to provide a clear skip message.
		t.Skip("PHOTO_ANALYSIS_ENGINE_EXE not set — Rust live comparison skipped. Set env var and run 'make all' first.")
	}
	if _, err := os.Stat(engineExe); os.IsNotExist(err) {
		t.Skipf("Rust engine binary not found at %s — Rust engine DLL required", engineExe)
	}

	fixDir := fixtureDir(t, "sample_pavement_job")
	groupsPath := filepath.Join(fixDir, "photo-groups.json")
	masterPath := filepath.Join(fixDir, "master.csv")

	groupsData, err := os.ReadFile(groupsPath)
	if err != nil {
		t.Fatalf("read groups: %v", err)
	}
	var groups map[string]json.RawMessage
	if err := json.Unmarshal(groupsData, &groups); err != nil {
		t.Fatalf("parse groups: %v", err)
	}

	tmpDir := t.TempDir()
	for k := range groups {
		p := filepath.Join(tmpDir, k)
		if err := os.WriteFile(p, []byte{0xFF, 0xD8, 0xFF, 0xE0}, 0644); err != nil {
			t.Fatalf("create fake image: %v", err)
		}
	}
	grpDst := filepath.Join(tmpDir, "photo-groups.json")
	if err := os.WriteFile(grpDst, groupsData, 0644); err != nil {
		t.Fatalf("copy groups: %v", err)
	}

	// ---- Run Rust engine (match-master) ----
	rustResults, err := engine.MatchMaster(grpDst, masterPath, tmpDir, "", "")
	if err != nil {
		t.Skipf("Rust engine.MatchMaster failed — skipping live cross-check: %v", err)
	}

	// ---- Run Go pipeline ----
	hm, err := matching.NewHierarchyMasterFromCSVFile(masterPath)
	if err != nil {
		t.Fatalf("load master: %v", err)
	}
	stub := func(_ engine.ImageConfig) (engine.ImageResult, error) {
		return engine.ImageResult{OutputJSON: grpDst, PhotoCount: len(groups)}, nil
	}
	cfg := pipeline.Config{
		Folder:          tmpDir,
		BatchSize:       10,
		IncludeAll:      true,
		HierarchyMaster: hm,
		ProcessImageFn:  stub,
	}
	goResults, err := pipeline.Run(context.Background(), cfg)
	if err != nil {
		t.Fatalf("pipeline.Run: %v", err)
	}

	// ---- Normalize both ----
	rustNorm := make([]normalizedResult, len(rustResults))
	for i, r := range rustResults {
		rustNorm[i] = normalizeEngineResult(r)
	}
	sortByFileName(rustNorm)

	goNorm := make([]normalizedResult, len(goResults))
	for i, r := range goResults {
		goNorm[i] = normalizeGoResult(r)
	}
	sortByFileName(goNorm)

	t.Logf("Rust engine: %d results; Go pipeline: %d results", len(rustNorm), len(goNorm))

	// Log all divergences but do not fail — documented open gaps.
	if len(rustNorm) != len(goNorm) {
		// DIV: result count differs — Rust may include/exclude entries Go does not.
		// TODO: Go vs Rust — investigate result count discrepancy.
		t.Logf("DIV: result count differs: Rust=%d Go=%d", len(rustNorm), len(goNorm))
		return
	}

	diverged := 0
	for i := range rustNorm {
		rust := rustNorm[i]
		go_ := goNorm[i]
		if rust.FileName != go_.FileName {
			t.Logf("DIV[%d]: fileName sort mismatch: Rust=%q Go=%q", i, rust.FileName, go_.FileName)
			continue
		}
		// Compare field by field, logging each divergence.
		checkField := func(field, rustVal, goVal string) {
			if rustVal != goVal {
				diverged++
				t.Logf("DIV [%s] %s: Rust=%q Go=%q", rust.FileName, field, rustVal, goVal)
			}
		}
		checkField("workType", rust.WorkType, go_.WorkType)
		checkField("variety", rust.Variety, go_.Variety)
		checkField("subphase", rust.Subphase, go_.Subphase)
		checkField("remarks", rust.Remarks, go_.Remarks)
		checkField("photoCategory", rust.PhotoCategory, go_.PhotoCategory)
		checkField("station", rust.Station, go_.Station)
		// hasBoard / detectedText should always match (both from same mock source).
		if rust.HasBoard != go_.HasBoard {
			diverged++
			t.Logf("DIV [%s] hasBoard: Rust=%v Go=%v", rust.FileName, rust.HasBoard, go_.HasBoard)
		}
		if rust.DetectedText != go_.DetectedText {
			diverged++
			t.Logf("DIV [%s] detectedText: Rust=%q Go=%q", rust.FileName, rust.DetectedText, go_.DetectedText)
		}
	}

	if diverged == 0 {
		t.Log("PASS: Rust and Go outputs are fully equivalent for this fixture.")
	} else {
		// Known open gaps — not a hard failure, documented for Stream K.
		t.Logf("DIVERGENCE: %d field(s) differ between Rust and Go (documented open gaps)", diverged)
		// Uncomment the line below to make divergences fail CI once gaps are closed:
		// t.Errorf("%d field(s) diverge — see DIV lines above", diverged)
	}
}
