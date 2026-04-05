package export

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"github.com/YuujiKamura/photo-ai-go/pkg/engine"
)

// DefaultResultFileName is the standard name for the intermediate analysis JSON.
const DefaultResultFileName = "result.json"

// SaveResults writes analysis results to a JSON file.
// If outputPath is a directory, result.json is written inside it.
// If outputPath is empty, result.json is written to inputFolder.
func SaveResults(results []engine.AnalysisResult, inputFolder, outputPath string) (string, error) {
	dest := resolveJSONPath(inputFolder, outputPath)

	if err := os.MkdirAll(filepath.Dir(dest), 0o755); err != nil {
		return "", fmt.Errorf("create output directory: %w", err)
	}

	data, err := json.MarshalIndent(results, "", "  ")
	if err != nil {
		return "", fmt.Errorf("marshal results: %w", err)
	}

	if err := os.WriteFile(dest, data, 0o644); err != nil {
		return "", fmt.Errorf("write %s: %w", dest, err)
	}

	return dest, nil
}

// LoadResults reads analysis results from a JSON file.
func LoadResults(jsonPath string) ([]engine.AnalysisResult, error) {
	data, err := os.ReadFile(jsonPath)
	if err != nil {
		return nil, fmt.Errorf("read %s: %w", jsonPath, err)
	}

	var results []engine.AnalysisResult
	if err := json.Unmarshal(data, &results); err != nil {
		return nil, fmt.Errorf("parse %s: %w", jsonPath, err)
	}

	return results, nil
}

// resolveJSONPath determines where to write result.json.
// outputPath may be empty, a directory, or a file path.
func resolveJSONPath(inputFolder, outputPath string) string {
	if outputPath == "" {
		return filepath.Join(inputFolder, DefaultResultFileName)
	}

	info, err := os.Stat(outputPath)
	if err == nil && info.IsDir() {
		return filepath.Join(outputPath, DefaultResultFileName)
	}

	// If it looks like a directory that doesn't exist yet, treat as directory.
	if filepath.Ext(outputPath) == "" {
		return filepath.Join(outputPath, DefaultResultFileName)
	}

	return outputPath
}

// ResolveExportPath determines the export output path (PDF or Excel file).
// If outputPath has no extension, it is treated as a directory and the default
// filename is appended. ext should include the leading dot, e.g. ".pdf".
func ResolveExportPath(inputJSON, outputPath, defaultName string) string {
	if outputPath == "" {
		return filepath.Join(filepath.Dir(inputJSON), defaultName)
	}

	info, err := os.Stat(outputPath)
	if err == nil && info.IsDir() {
		return filepath.Join(outputPath, defaultName)
	}

	// Non-existent path with no extension → treat as directory.
	if filepath.Ext(outputPath) == "" {
		return filepath.Join(outputPath, defaultName)
	}

	return outputPath
}
