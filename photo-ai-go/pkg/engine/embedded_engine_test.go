//go:build windows

package engine

import (
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestEmbeddedAnalysisEngineSupportsMatchMaster(t *testing.T) {
	extractedDir, err := EnsureEngines()
	if err != nil {
		t.Fatalf("EnsureEngines failed: %v", err)
	}

	enginePath := filepath.Join(extractedDir, "photo-analysis-engine.exe")
	cmd := exec.Command(enginePath, "--help")
	output, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("running %s --help failed: %v\noutput: %s", enginePath, err, string(output))
	}

	helpText := string(output)
	if !strings.Contains(helpText, "match-master") {
		t.Fatalf("embedded analysis engine is missing match-master support\noutput: %s", helpText)
	}
}

func TestEmbeddedAnalysisEngineMatchMasterSmoke(t *testing.T) {
	extractedDir, err := EnsureEngines()
	if err != nil {
		t.Fatalf("EnsureEngines failed: %v", err)
	}
	t.Setenv("PHOTO_ANALYSIS_ENGINE_EXE", filepath.Join(extractedDir, "photo-analysis-engine.exe"))

	tempDir := t.TempDir()
	inputJSON := filepath.Join(tempDir, "photo-groups.json")
	masterCSV := filepath.Join(tempDir, "master.csv")
	photoPath := filepath.Join(tempDir, "IMG_0001.JPG")

	if err := os.WriteFile(photoPath, []byte("not-an-image"), 0o644); err != nil {
		t.Fatalf("write photo placeholder: %v", err)
	}

	groupRecords := map[string]any{
		"IMG_0001.JPG": map[string]any{
			"role":          "overview",
			"machine_type":  "バックホウ",
			"machine_id":    "0.45",
			"has_board":     true,
			"detected_text": "No.1 舗装工",
			"description":   "舗装工の施工状況",
			"group":         1,
		},
	}
	data, err := json.Marshal(groupRecords)
	if err != nil {
		t.Fatalf("marshal group records: %v", err)
	}
	if err := os.WriteFile(inputJSON, data, 0o644); err != nil {
		t.Fatalf("write input json: %v", err)
	}

	csv := strings.Join([]string{
		"費目,写真区分,工種,種別,細別,備考,検索パターン",
		"共通,施工状況写真,舗装工,表層工,施工状況,No.1,舗装工",
	}, "\n")
	if err := os.WriteFile(masterCSV, []byte(csv), 0o644); err != nil {
		t.Fatalf("write master csv: %v", err)
	}

	results, err := MatchMaster(inputJSON, masterCSV, tempDir, "", "")
	if err != nil {
		t.Fatalf("MatchMaster failed: %v", err)
	}
	if len(results) != 1 {
		t.Fatalf("expected exactly 1 result, got %d", len(results))
	}
}
