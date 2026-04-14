package pipeline

// pipeline_end_to_end_test.go — table-driven integration test for Run().
//
// Mocks engine.ProcessImage via Config.ProcessImageFn so the full pipeline
// executes without a real AI backend, Gemini API, or Rust engine binary.
//
// Test fixtures use ダミー names only (no proprietary names).

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/YuujiKamura/photo-ai-go/pkg/engine"
)

// dummyGroupRecord mirrors pkg/tagger.GroupRecord for JSON encoding in fixtures.
type dummyGroupRecord struct {
	Role         string `json:"role"`
	MachineType  string `json:"machine_type"`
	MachineID    string `json:"machine_id"`
	HasBoard     bool   `json:"has_board"`
	DetectedText string `json:"detected_text,omitempty"`
	Description  string `json:"description,omitempty"`
	Group        uint32 `json:"group"`
}

// writeGroupsJSON writes a photo-groups.json fixture to dir and returns its path.
func writeGroupsJSON(t *testing.T, dir string, records map[string]dummyGroupRecord) string {
	t.Helper()
	data, err := json.MarshalIndent(records, "", "  ")
	if err != nil {
		t.Fatalf("marshal groups: %v", err)
	}
	p := filepath.Join(dir, "photo-groups.json")
	if err := os.WriteFile(p, data, 0644); err != nil {
		t.Fatalf("write groups: %v", err)
	}
	return p
}

// createFakeImages creates empty files with the given names in dir.
func createFakeImages(t *testing.T, dir string, names ...string) {
	t.Helper()
	for _, name := range names {
		p := filepath.Join(dir, name)
		if err := os.WriteFile(p, []byte{}, 0644); err != nil {
			t.Fatalf("create image %s: %v", name, err)
		}
	}
}

// TestPipelineEndToEnd exercises Run() with a mocked ProcessImageFn.
func TestPipelineEndToEnd(t *testing.T) {
	cases := []struct {
		name               string
		imageFiles         []string // filenames to create in the temp dir
		groupRecords       map[string]dummyGroupRecord
		wantDetectedTexts  map[string]string // filename → expected DetectedText
		wantCandidatesFor  string            // filename to check WorkTypeCandidates on
		wantCandidateIn    []string          // at least one of these must appear in WorkTypeCandidates
	}{
		{
			name:       "pavement_photos_with_board",
			imageFiles: []string{"ダミー001.JPG", "ダミー002.JPG"},
			groupRecords: map[string]dummyGroupRecord{
				"ダミー001.JPG": {
					HasBoard:     true,
					DetectedText: "舗装工 アスファルト",
					Description:  "アスファルトフィニッシャーで舗設中",
					Group:        1,
				},
				"ダミー002.JPG": {
					HasBoard:     true,
					DetectedText: "到着温度 160℃",
					Description:  "黒板アップ",
					Group:        1,
				},
			},
			wantDetectedTexts: map[string]string{
				"ダミー001.JPG": "舗装工 アスファルト",
				"ダミー002.JPG": "到着温度 160℃",
			},
			wantCandidatesFor: "ダミー001.JPG",
			wantCandidateIn:   []string{"舗装工"},
		},
		{
			name:       "marking_photos",
			imageFiles: []string{"ダミーA.JPG"},
			groupRecords: map[string]dummyGroupRecord{
				"ダミーA.JPG": {
					HasBoard:     false,
					DetectedText: "区画線施工",
					Description:  "白線を引いている現場",
					Group:        1,
				},
			},
			wantDetectedTexts: map[string]string{
				"ダミーA.JPG": "区画線施工",
			},
			wantCandidatesFor: "ダミーA.JPG",
			wantCandidateIn:   []string{"区画線工"},
		},
		{
			name:       "no_board_no_text",
			imageFiles: []string{"ダミーX.JPG"},
			groupRecords: map[string]dummyGroupRecord{
				"ダミーX.JPG": {
					HasBoard:    false,
					Description: "ただの風景",
					Group:       1,
				},
			},
			wantDetectedTexts: map[string]string{
				"ダミーX.JPG": "",
			},
			wantCandidatesFor: "ダミーX.JPG",
			// No keywords match → WorkTypeCandidates may be empty (no assertion needed)
			wantCandidateIn: nil,
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			// Create temp dir with fake image files.
			dir := t.TempDir()
			createFakeImages(t, dir, tc.imageFiles...)

			// Write photo-groups.json fixture.
			groupsPath := writeGroupsJSON(t, dir, tc.groupRecords)

			// Build the ProcessImageFn stub: returns groupsPath as OutputJSON.
			stub := func(cfg engine.ImageConfig) (engine.ImageResult, error) {
				return engine.ImageResult{
					OutputJSON: groupsPath,
					PhotoCount: len(tc.groupRecords),
				}, nil
			}

			cfg := Config{
				Folder:         dir,
				BatchSize:      10,
				IncludeAll:     true, // accept all extensions including .JPG
				ProcessImageFn: stub,
			}

			results, err := Run(context.Background(), cfg)
			if err != nil {
				t.Fatalf("Run() error: %v", err)
			}

			// Build a lookup by filename.
			byName := make(map[string]Result, len(results))
			for _, r := range results {
				byName[r.FileName] = r
			}

			// Assert DetectedText was populated.
			for fname, want := range tc.wantDetectedTexts {
				r, ok := byName[fname]
				if !ok {
					t.Errorf("result for %s not found; got %v", fname, resultNames(results))
					continue
				}
				if r.DetectedText != want {
					t.Errorf("%s DetectedText = %q, want %q", fname, r.DetectedText, want)
				}
			}

			// Assert WorkTypeCandidates when expected.
			if tc.wantCandidateIn != nil {
				r, ok := byName[tc.wantCandidatesFor]
				if !ok {
					t.Errorf("result for %s not found", tc.wantCandidatesFor)
				} else {
					found := false
					for _, want := range tc.wantCandidateIn {
						for _, got := range r.WorkTypeCandidates {
							if got == want {
								found = true
								break
							}
						}
					}
					if !found {
						t.Errorf("%s WorkTypeCandidates = %v, want at least one of %v",
							tc.wantCandidatesFor, r.WorkTypeCandidates, tc.wantCandidateIn)
					}
				}
			}
		})
	}
}

// TestPipelineEndToEnd_ProcessImageError verifies that ProcessImage errors propagate.
func TestPipelineEndToEnd_ProcessImageError(t *testing.T) {
	dir := t.TempDir()
	createFakeImages(t, dir, "ダミー001.JPG")

	stub := func(_ engine.ImageConfig) (engine.ImageResult, error) {
		return engine.ImageResult{}, os.ErrNotExist
	}

	cfg := Config{
		Folder:         dir,
		IncludeAll:     true,
		ProcessImageFn: stub,
	}

	_, err := Run(context.Background(), cfg)
	if err == nil {
		t.Fatal("expected error from ProcessImageFn, got nil")
	}
}

// resultNames returns a slice of FileName strings from results (for diagnostics).
func resultNames(results []Result) []string {
	names := make([]string, len(results))
	for i, r := range results {
		names[i] = r.FileName
	}
	return names
}
