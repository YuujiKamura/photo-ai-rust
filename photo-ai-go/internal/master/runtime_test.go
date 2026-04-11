package master

import (
	"os"
	"path/filepath"
	"testing"
)

func TestResolveMasterSourceSeedsUserRoot(t *testing.T) {
	tmp := t.TempDir()
	t.Setenv("PHOTO_AI_MASTER_DIR", filepath.Join(tmp, "user-master"))
	src, err := ResolveMasterSource(t.TempDir())
	if err != nil {
		t.Fatalf("ResolveMasterSource: %v", err)
	}
	if src.Source != SourceUser {
		t.Fatalf("expected user source, got %s", src.Source)
	}
	if _, err := os.Stat(filepath.Join(src.RootDir, "manifest.json")); err != nil {
		t.Fatalf("expected seeded manifest: %v", err)
	}
	if _, err := os.Stat(filepath.Join(src.RootDir, "by_work_type", "舗装工.csv")); err != nil {
		t.Fatalf("expected seeded csv: %v", err)
	}
}
