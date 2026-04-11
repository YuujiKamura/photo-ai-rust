//go:build windows

package engine

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestResolveEnginePathFromEnv(t *testing.T) {
	t.Setenv("PHOTO_TEST_RESOLVE_EXE", `C:\fake\engine.exe`)
	resolved, err := resolveEnginePath("PHOTO_TEST_RESOLVE_EXE", "engine.exe")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if resolved.Source != "env" {
		t.Errorf("expected source=env, got %q", resolved.Source)
	}
	if resolved.Path != `C:\fake\engine.exe` {
		t.Errorf("expected path from env, got %q", resolved.Path)
	}
}

func TestResolveEnginePathFallback(t *testing.T) {
	// Unset env var so it falls through all resolution steps
	t.Setenv("PHOTO_RESOLVE_NONEXISTENT_EXE", "")
	resolved, err := resolveEnginePath("PHOTO_RESOLVE_NONEXISTENT_EXE", "nonexistent-engine.exe")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if resolved.Source != "path-fallback" {
		t.Errorf("expected source=path-fallback, got %q", resolved.Source)
	}
	if resolved.Path != "nonexistent-engine.exe" {
		t.Errorf("expected bare name fallback, got %q", resolved.Path)
	}
}

func TestMatchMasterErrorContainsEngineInfo(t *testing.T) {
	// Point to a fake exe that doesn't exist — execution must fail and
	// the error message must include the resolved path and source.
	fakeExe := filepath.Join(t.TempDir(), "fake-analysis-engine.exe")
	t.Setenv("PHOTO_ANALYSIS_ENGINE_EXE", fakeExe)

	_, err := MatchMaster("input.json", "master.csv", t.TempDir(), "", "")
	if err == nil {
		t.Fatal("expected error from MatchMaster with fake engine, got nil")
	}

	errMsg := err.Error()
	if !strings.Contains(errMsg, fakeExe) {
		t.Errorf("error should contain engine path %q, got: %s", fakeExe, errMsg)
	}
	if !strings.Contains(errMsg, "source=env") {
		t.Errorf("error should contain source=env, got: %s", errMsg)
	}
	if !strings.Contains(errMsg, "match-master failed") {
		t.Errorf("error should contain 'match-master failed', got: %s", errMsg)
	}
}

func TestNormalizeErrorContainsEngineInfo(t *testing.T) {
	fakeExe := filepath.Join(t.TempDir(), "fake-analysis-engine.exe")
	t.Setenv("PHOTO_ANALYSIS_ENGINE_EXE", fakeExe)

	_, err := Normalize("input.json", t.TempDir(), "", "")
	if err == nil {
		t.Fatal("expected error from Normalize with fake engine, got nil")
	}

	errMsg := err.Error()
	if !strings.Contains(errMsg, fakeExe) {
		t.Errorf("error should contain engine path %q, got: %s", fakeExe, errMsg)
	}
	if !strings.Contains(errMsg, "source=env") {
		t.Errorf("error should contain source=env, got: %s", errMsg)
	}
	if !strings.Contains(errMsg, "normalize failed") {
		t.Errorf("error should contain 'normalize failed', got: %s", errMsg)
	}
}

func TestMatchMasterBadEngineOutputContainsEngineInfo(t *testing.T) {
	// Create a fake exe that exits with error output (a batch script).
	tempDir := t.TempDir()
	// Write a minimal Windows executable stub — a .cmd wrapper works via exec.Command
	fakeExe := filepath.Join(tempDir, "fake-engine.cmd")
	script := "@echo off\necho {\"ok\":false,\"error\":\"simulated engine failure\"}\nexit /b 1\n"
	if err := os.WriteFile(fakeExe, []byte(script), 0o755); err != nil {
		t.Fatalf("write fake engine: %v", err)
	}
	t.Setenv("PHOTO_ANALYSIS_ENGINE_EXE", fakeExe)

	_, err := MatchMaster("input.json", "master.csv", tempDir, "", "")
	if err == nil {
		t.Fatal("expected error from MatchMaster with bad engine, got nil")
	}

	errMsg := err.Error()
	if !strings.Contains(errMsg, fakeExe) {
		t.Errorf("error should contain engine path %q, got: %s", fakeExe, errMsg)
	}
	if !strings.Contains(errMsg, "source=env") {
		t.Errorf("error should contain source=env, got: %s", errMsg)
	}
}
