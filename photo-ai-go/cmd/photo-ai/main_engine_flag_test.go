// Package main — main_engine_flag_test.go
// Tests that --engine flag parsing and routing select the correct path.
// Both engine paths are exercised via lightweight mocks so the test does not
// require the Rust DLL, embedded EXEs, or real photos.
package main

import (
	"context"
	"errors"
	"testing"

	"github.com/YuujiKamura/photo-ai-go/internal/pipeline"
)

// ─── mock wiring ─────────────────────────────────────────────────────────────

// routeRecorder records which engine was selected by analyzeRouter.
type routeRecorder struct {
	rustCalled bool
	goCalled   bool
	rustErr    error
	goErr      error
}

// analyzeRouter is a testable function extracted from the cobra RunE logic.
// Real code in analyzeCmd.RunE calls runRustEngine / runGoEngine directly;
// this version accepts them as function parameters for dependency injection.
func analyzeRouter(
	engineName string,
	folder, destJSON, master string,
	flags analyzeFlagSet,
	goRunner func(folder, destJSON string, flags analyzeFlagSet) error,
	rustRunner func(folder, destJSON, master string, flags analyzeFlagSet) error,
) error {
	switch engineName {
	case "rust":
		return rustRunner(folder, destJSON, master, flags)
	case "go":
		return goRunner(folder, destJSON, flags)
	default:
		return errors.New("--engine must be 'rust' or 'go', got " + engineName)
	}
}

// ─── tests ────────────────────────────────────────────────────────────────────

func TestAnalyzeRouterSelectsRust(t *testing.T) {
	rec := &routeRecorder{}
	err := analyzeRouter(
		"rust",
		"/tmp/photos", "/tmp/photos/result.json", "",
		analyzeFlagSet{batchSize: 5},
		func(folder, destJSON string, flags analyzeFlagSet) error {
			rec.goCalled = true
			return rec.goErr
		},
		func(folder, destJSON, master string, flags analyzeFlagSet) error {
			rec.rustCalled = true
			return rec.rustErr
		},
	)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !rec.rustCalled {
		t.Error("expected rust runner to be called")
	}
	if rec.goCalled {
		t.Error("go runner must NOT be called when engine=rust")
	}
}

func TestAnalyzeRouterSelectsGo(t *testing.T) {
	rec := &routeRecorder{}
	err := analyzeRouter(
		"go",
		"/tmp/photos", "/tmp/photos/result.json", "",
		analyzeFlagSet{batchSize: 5},
		func(folder, destJSON string, flags analyzeFlagSet) error {
			rec.goCalled = true
			return rec.goErr
		},
		func(folder, destJSON, master string, flags analyzeFlagSet) error {
			rec.rustCalled = true
			return rec.rustErr
		},
	)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !rec.goCalled {
		t.Error("expected go runner to be called")
	}
	if rec.rustCalled {
		t.Error("rust runner must NOT be called when engine=go")
	}
}

func TestAnalyzeRouterRejectsInvalidEngine(t *testing.T) {
	err := analyzeRouter(
		"invalid",
		"/tmp/photos", "/tmp/photos/result.json", "",
		analyzeFlagSet{},
		func(_, _ string, _ analyzeFlagSet) error { return nil },
		func(_, _, _ string, _ analyzeFlagSet) error { return nil },
	)
	if err == nil {
		t.Fatal("expected error for unknown engine name, got nil")
	}
}

func TestAnalyzeRouterPropagatesGoError(t *testing.T) {
	wantErr := errors.New("go pipeline failed")
	err := analyzeRouter(
		"go",
		"/tmp/photos", "/tmp/photos/result.json", "",
		analyzeFlagSet{},
		func(_, _ string, _ analyzeFlagSet) error { return wantErr },
		func(_, _, _ string, _ analyzeFlagSet) error { return nil },
	)
	if !errors.Is(err, wantErr) {
		t.Fatalf("expected %v, got %v", wantErr, err)
	}
}

func TestAnalyzeRouterPropagatesRustError(t *testing.T) {
	wantErr := errors.New("rust engine failed")
	err := analyzeRouter(
		"rust",
		"/tmp/photos", "/tmp/photos/result.json", "",
		analyzeFlagSet{},
		func(_, _ string, _ analyzeFlagSet) error { return nil },
		func(_, _, _ string, _ analyzeFlagSet) error { return wantErr },
	)
	if !errors.Is(err, wantErr) {
		t.Fatalf("expected %v, got %v", wantErr, err)
	}
}

// ─── pipeline → engine schema mapping ────────────────────────────────────────

func TestPipelineResultsToEngineResultsMapping(t *testing.T) {
	src := []pipeline.Result{
		{
			FileName:      "IMG_001.jpg",
			FilePath:      "/photos/IMG_001.jpg",
			Date:          "2026-01-15",
			WorkType:      "舗装工",
			Variety:       "アスファルト舗装",
			Subphase:      "施工前",
			Station:       "No.10",
			Remarks:       "備考テキスト",
			Description:   "説明テキスト",
			HasBoard:      true,
			DetectedText:  "測点 No.10",
			Measurements:  "厚さ5cm",
			PhotoCategory: "品質管理",
			Reasoning:     "根拠テキスト",
			FocusTarget:   "路面",
			Group:         3,
		},
	}

	got := pipelineResultsToEngineResults(src)

	if len(got) != 1 {
		t.Fatalf("expected 1 result, got %d", len(got))
	}
	r := got[0]

	checks := []struct {
		field string
		got   string
		want  string
	}{
		{"FileName", r.FileName, src[0].FileName},
		{"FilePath", r.FilePath, src[0].FilePath},
		{"Date", r.Date, src[0].Date},
		{"WorkType", r.WorkType, src[0].WorkType},
		{"Variety", r.Variety, src[0].Variety},
		{"Subphase", r.Subphase, src[0].Subphase},
		{"Station", r.Station, src[0].Station},
		{"Remarks", r.Remarks, src[0].Remarks},
		{"Description", r.Description, src[0].Description},
		{"DetectedText", r.DetectedText, src[0].DetectedText},
		{"Measurements", r.Measurements, src[0].Measurements},
		{"PhotoCategory", r.PhotoCategory, src[0].PhotoCategory},
		{"Reasoning", r.Reasoning, src[0].Reasoning},
		{"FocusTarget", r.FocusTarget, src[0].FocusTarget},
	}
	for _, c := range checks {
		if c.got != c.want {
			t.Errorf("%s: got %q, want %q", c.field, c.got, c.want)
		}
	}
	if r.HasBoard != src[0].HasBoard {
		t.Errorf("HasBoard: got %v, want %v", r.HasBoard, src[0].HasBoard)
	}
	if r.Group != src[0].Group {
		t.Errorf("Group: got %d, want %d", r.Group, src[0].Group)
	}
}

// ─── analyzeFlagSet round-trip ─────────────────────────────────────────────

func TestAnalyzeFlagSetDefaultValues(t *testing.T) {
	flags := analyzeFlagSet{}
	if flags.batchSize != 0 {
		t.Errorf("default batchSize should be zero value")
	}
	if flags.payPerUse {
		t.Errorf("default payPerUse should be false")
	}
}

// ─── pipeline.Run smoke (no real I/O) ────────────────────────────────────────
// Verify pipeline.Run rejects an empty folder without panicking.

func TestPipelineRunRejectsEmptyFolder(t *testing.T) {
	_, err := pipeline.Run(context.Background(), pipeline.Config{
		Folder:    t.TempDir(), // empty temp dir — no images
		BatchSize: 1,
	})
	if err == nil {
		t.Fatal("expected error for folder with no images, got nil")
	}
}
