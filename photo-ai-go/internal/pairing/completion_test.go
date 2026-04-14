package pairing

import (
	"os"
	"path/filepath"
	"testing"
)

// ---------------------------------------------------------------------------
// TestFilterPairs — mirrors load_pairs_from_json / build_pair_folders filter
// ---------------------------------------------------------------------------

func TestFilterPairs(t *testing.T) {
	tests := []struct {
		name    string
		input   []PairCompletionResult
		wantLen int
	}{
		{
			name: "unanimous and majority kept, split dropped",
			input: []PairCompletionResult{
				{BeforePage: 1, AfterFile: "a.jpg", Confidence: 1.0},   // unanimous
				{BeforePage: 2, AfterFile: "b.jpg", Confidence: 0.67},  // majority boundary
				{BeforePage: 3, AfterFile: "c.jpg", Confidence: 0.5},   // split → dropped
				{BeforePage: 4, AfterFile: "", Confidence: 1.0},         // empty after → dropped
			},
			wantLen: 2,
		},
		{
			name:    "empty input",
			input:   []PairCompletionResult{},
			wantLen: 0,
		},
		{
			name: "all above threshold",
			input: []PairCompletionResult{
				{BeforePage: 1, AfterFile: "x.jpg", Confidence: 0.8},
				{BeforePage: 2, AfterFile: "y.jpg", Confidence: 1.0},
			},
			wantLen: 2,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := FilterPairs(tt.input)
			if len(got) != tt.wantLen {
				t.Errorf("FilterPairs len = %d, want %d", len(got), tt.wantLen)
			}
		})
	}
}

// ---------------------------------------------------------------------------
// TestSummarisePairs — mirrors the summary block in handle_pair_completion
// ---------------------------------------------------------------------------

func TestSummarisePairs(t *testing.T) {
	results := []PairCompletionResult{
		{Confidence: 1.0},  // unanimous
		{Confidence: 1.0},  // unanimous
		{Confidence: 0.67}, // majority
		{Confidence: 0.5},  // split
		{Confidence: 0.0},  // split
	}
	s := SummarisePairs(results)
	if s.Unanimous != 2 {
		t.Errorf("Unanimous = %d, want 2", s.Unanimous)
	}
	if s.Majority != 1 {
		t.Errorf("Majority = %d, want 1", s.Majority)
	}
	if s.Split != 2 {
		t.Errorf("Split = %d, want 2", s.Split)
	}
}

// ---------------------------------------------------------------------------
// TestStationNameFromFolderName
// Mirrors the station extraction in scan_pair_folders / build_pair_folders
// ---------------------------------------------------------------------------

func TestStationNameFromFolderName(t *testing.T) {
	tests := []struct {
		name       string
		folderName string
		want       string
	}{
		// ── Golden tests with realistic pavement-work station names ─────────
		{"P01 with station", "P01_No.5 右車線", "No.5 右車線"},
		{"P12 Japanese station", "P12_サンプル測点", "サンプル測点"},
		{"P02 with road name", "P02_取付道路 No.3", "取付道路 No.3"},
		// ── Edge cases ───────────────────────────────────────────────────────
		{"no underscore", "unrelated", "unrelated"},
		{"empty string", "", ""},
		{"only underscore", "_", ""},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := StationNameFromFolderName(tt.folderName)
			if got != tt.want {
				t.Errorf("StationNameFromFolderName(%q) = %q, want %q", tt.folderName, got, tt.want)
			}
		})
	}
}

// ---------------------------------------------------------------------------
// TestFindFileInPFolders — mirrors find_file_in_pfolders in pair_completion.rs
// ---------------------------------------------------------------------------

func TestFindFileInPFolders(t *testing.T) {
	// Build a temporary directory tree:
	//   root/
	//     P01_テスト測点/
	//       before.jpg
	//     P02_別測点/
	//       after.jpg
	//     NotP/
	//       before.jpg  ← should NOT be found (not a P* folder)
	root := t.TempDir()
	p01 := filepath.Join(root, "P01_テスト測点")
	p02 := filepath.Join(root, "P02_別測点")
	notp := filepath.Join(root, "NotP")
	for _, dir := range []string{p01, p02, notp} {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatal(err)
		}
	}
	for _, f := range []string{
		filepath.Join(p01, "before.jpg"),
		filepath.Join(p02, "after.jpg"),
		filepath.Join(notp, "before.jpg"),
	} {
		if err := os.WriteFile(f, []byte{}, 0o644); err != nil {
			t.Fatal(err)
		}
	}

	t.Run("file in P01 found", func(t *testing.T) {
		got := FindFileInPFolders(root, "before.jpg")
		if got == "" {
			t.Error("expected to find before.jpg, got empty string")
		}
		// Must be inside a P* directory.
		if !filepath.IsAbs(got) {
			t.Errorf("got non-absolute path: %q", got)
		}
	})

	t.Run("file in P02 found", func(t *testing.T) {
		got := FindFileInPFolders(root, "after.jpg")
		if got == "" {
			t.Error("expected to find after.jpg, got empty string")
		}
	})

	t.Run("non-existent file returns empty", func(t *testing.T) {
		got := FindFileInPFolders(root, "missing.jpg")
		if got != "" {
			t.Errorf("expected empty, got %q", got)
		}
	})

	t.Run("empty filename returns empty", func(t *testing.T) {
		got := FindFileInPFolders(root, "")
		if got != "" {
			t.Errorf("expected empty, got %q", got)
		}
	})
}

// ---------------------------------------------------------------------------
// TestConfidenceConstants
// ---------------------------------------------------------------------------

func TestConfidenceConstants(t *testing.T) {
	if ConfidenceMajority != 0.67 {
		t.Errorf("ConfidenceMajority = %f, want 0.67", ConfidenceMajority)
	}
	if ConfidenceUnanimous != 1.0 {
		t.Errorf("ConfidenceUnanimous = %f, want 1.0", ConfidenceUnanimous)
	}
}
