package tagger

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

const groupFile = "photo-groups.json"

// isImage returns true if the path has an image extension.
func isImage(p string) bool {
	ext := strings.ToLower(filepath.Ext(p))
	switch ext {
	case ".jpg", ".jpeg", ".png", ".heic":
		return true
	}
	return false
}

// LoadGroupRecords loads existing photo-groups.json from the folder.
// Returns an empty map if the file doesn't exist or is invalid.
func LoadGroupRecords(base string) GroupRecords {
	path := filepath.Join(base, groupFile)
	data, err := os.ReadFile(path)
	if err != nil {
		return make(GroupRecords)
	}
	var records GroupRecords
	if err := json.Unmarshal(data, &records); err != nil {
		return make(GroupRecords)
	}
	if records == nil {
		return make(GroupRecords)
	}
	return records
}

// SaveGroupRecords writes photo-groups.json to the folder.
func SaveGroupRecords(base string, records GroupRecords) error {
	path := filepath.Join(base, groupFile)
	data, err := json.MarshalIndent(records, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to serialize group records: %w", err)
	}
	if err := os.WriteFile(path, data, 0644); err != nil {
		return fmt.Errorf("failed to write %s: %w", path, err)
	}
	return nil
}

// CollectImagesFlat collects image files directly under dir (non-recursive), sorted.
func CollectImagesFlat(dir string) []string {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil
	}
	var out []string
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		p := filepath.Join(dir, e.Name())
		if isImage(p) {
			out = append(out, p)
		}
	}
	sort.Strings(out)
	return out
}
