package master

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
)

const (
	SourceUser         = "user"
	SourceWorkspace    = "workspace"
	SourceEmbeddedSeed = "embedded-seed"
)

type Source struct {
	RootDir       string
	Source        string
	Writable      bool
	Manifest      Manifest
	SchemaVersion int
	MasterVersion string
}

func ResolveMasterSource(repoDir string) (Source, error) {
	userRoot, err := userMasterRoot()
	if err == nil {
		if err := ensureSeededUserMaster(userRoot); err == nil {
			if src, ok := loadSourceFromRoot(userRoot, SourceUser, true); ok {
				return src, nil
			}
		}
	}

	if src, ok := loadSourceFromRoot(filepath.Join(repoDir, "master"), SourceWorkspace, true); ok {
		return src, nil
	}

	return Source{}, fmt.Errorf("no usable master source found")
}

func userMasterRoot() (string, error) {
	base := os.Getenv("PHOTO_AI_MASTER_DIR")
	if base == "" {
		base = os.Getenv("LOCALAPPDATA")
		if base == "" {
			var err error
			base, err = os.UserConfigDir()
			if err != nil {
				return "", fmt.Errorf("resolve user config dir: %w", err)
			}
		}
		base = filepath.Join(base, "photo-ai", "master")
	}
	return base, nil
}

func ensureSeededUserMaster(root string) error {
	manifestPath := filepath.Join(root, "manifest.json")
	if _, err := os.Stat(manifestPath); err == nil {
		return nil
	}

	manifest, err := ReadEmbeddedManifest()
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Join(root, "by_work_type"), 0o755); err != nil {
		return fmt.Errorf("mkdir user master root: %w", err)
	}
	for _, name := range manifest.Files {
		data, err := EmbeddedMaster.ReadFile(name)
		if err != nil {
			return fmt.Errorf("read embedded master file %s: %w", name, err)
		}
		dest := filepath.Join(root, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(dest), 0o755); err != nil {
			return fmt.Errorf("mkdir parent for %s: %w", dest, err)
		}
		if err := os.WriteFile(dest, data, 0o644); err != nil {
			return fmt.Errorf("write seeded master file %s: %w", dest, err)
		}
	}
	if err := WriteManifestToRoot(root, manifest); err != nil {
		return err
	}
	return nil
}

func loadSourceFromRoot(root string, sourceName string, writable bool) (Source, bool) {
	manifest, err := ReadManifestFromRoot(root)
	if err != nil {
		return Source{}, false
	}
	for _, name := range manifest.Files {
		if _, err := os.Stat(filepath.Join(root, filepath.FromSlash(name))); err != nil {
			return Source{}, false
		}
	}
	return Source{
		RootDir:       root,
		Source:        sourceName,
		Writable:      writable,
		Manifest:      manifest,
		SchemaVersion: manifest.SchemaVersion,
		MasterVersion: manifest.MasterVersion,
	}, true
}

func ListEmbeddedFiles() ([]string, error) {
	entries, err := fs.ReadDir(EmbeddedMaster, "by_work_type")
	if err != nil {
		return nil, err
	}
	files := make([]string, 0, len(entries))
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		files = append(files, filepath.ToSlash(filepath.Join("by_work_type", entry.Name())))
	}
	return files, nil
}
