//go:build windows

package engine

import (
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
