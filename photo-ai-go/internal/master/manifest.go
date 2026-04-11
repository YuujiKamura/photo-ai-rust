package master

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

type Manifest struct {
	SchemaVersion int      `json:"schema_version"`
	MasterVersion string   `json:"master_version"`
	Files         []string `json:"files"`
}

func ReadEmbeddedManifest() (Manifest, error) {
	data, err := EmbeddedMaster.ReadFile("manifest.json")
	if err != nil {
		return Manifest{}, fmt.Errorf("read embedded manifest: %w", err)
	}
	return parseManifest(data)
}

func ReadManifestFromRoot(root string) (Manifest, error) {
	data, err := os.ReadFile(filepath.Join(root, "manifest.json"))
	if err != nil {
		return Manifest{}, fmt.Errorf("read manifest from root: %w", err)
	}
	return parseManifest(data)
}

func WriteManifestToRoot(root string, manifest Manifest) error {
	data, err := json.MarshalIndent(manifest, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal manifest: %w", err)
	}
	data = append(data, '\n')
	if err := os.WriteFile(filepath.Join(root, "manifest.json"), data, 0o644); err != nil {
		return fmt.Errorf("write manifest: %w", err)
	}
	return nil
}

func parseManifest(data []byte) (Manifest, error) {
	var manifest Manifest
	if err := json.Unmarshal(data, &manifest); err != nil {
		return Manifest{}, fmt.Errorf("parse manifest: %w", err)
	}
	if manifest.SchemaVersion <= 0 {
		return Manifest{}, fmt.Errorf("manifest schema_version must be > 0")
	}
	if manifest.MasterVersion == "" {
		return Manifest{}, fmt.Errorf("manifest master_version must not be empty")
	}
	if len(manifest.Files) == 0 {
		return Manifest{}, fmt.Errorf("manifest files must not be empty")
	}
	return manifest, nil
}
