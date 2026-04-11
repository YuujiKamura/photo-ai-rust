package master

import "testing"

func TestReadEmbeddedManifest(t *testing.T) {
	manifest, err := ReadEmbeddedManifest()
	if err != nil {
		t.Fatalf("ReadEmbeddedManifest: %v", err)
	}
	if manifest.SchemaVersion != 1 {
		t.Fatalf("unexpected schema version: %d", manifest.SchemaVersion)
	}
	if manifest.MasterVersion == "" {
		t.Fatal("master version should not be empty")
	}
	if len(manifest.Files) == 0 {
		t.Fatal("manifest files should not be empty")
	}
}

func TestEmbeddedManifestMatchesEmbeddedFiles(t *testing.T) {
	manifest, err := ReadEmbeddedManifest()
	if err != nil {
		t.Fatalf("ReadEmbeddedManifest: %v", err)
	}
	embeddedFiles, err := ListEmbeddedFiles()
	if err != nil {
		t.Fatalf("ListEmbeddedFiles: %v", err)
	}
	if len(manifest.Files) != len(embeddedFiles) {
		t.Fatalf("manifest/files mismatch: %d vs %d", len(manifest.Files), len(embeddedFiles))
	}
	expected := make(map[string]bool, len(manifest.Files))
	for _, name := range manifest.Files {
		expected[name] = true
	}
	for _, name := range embeddedFiles {
		if !expected[name] {
			t.Fatalf("embedded file missing from manifest: %s", name)
		}
	}
}
