package export

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/YuujiKamura/photo-ai-go/pkg/engine"
)

// GeneratePDF builds a PDFConfig from analysis results and calls the engine DLL.
//
// Parameters:
//   - inputJSON:    path to the result.json file (passed directly to the DLL)
//   - outputPath:   desired output path; if empty or a directory, a filename is derived
//   - photosPerPage: 2 or 3 (default: 3)
//   - quality:      "high" | "medium" | "low" (default: "medium")
//   - preset:       alias preset name, e.g. "pavement" (may be empty)
//   - aliasFile:    path to a custom alias JSON file (may be empty)
func GeneratePDF(inputJSON, outputPath string, photosPerPage int, quality, preset, aliasFile string) (engine.PDFResult, error) {
	if photosPerPage != 2 && photosPerPage != 3 {
		photosPerPage = 3
	}

	if quality == "" {
		quality = "medium"
	}

	defaultName := derivePDFName(inputJSON, photosPerPage)
	resolvedOutput := ResolveExportPath(inputJSON, outputPath, defaultName)

	cfg := engine.PDFConfig{
		InputJSON:     inputJSON,
		OutputPath:    resolvedOutput,
		PhotosPerPage: photosPerPage,
		Quality:       quality,
		Preset:        preset,
		AliasFile:     aliasFile,
	}

	result, err := engine.GeneratePDF(cfg)
	if err != nil {
		return result, fmt.Errorf("GeneratePDF: %w", err)
	}

	return result, nil
}

// derivePDFName builds a default output PDF filename from the input JSON path.
// e.g. "result.json" → "result_3up.pdf"
func derivePDFName(inputJSON string, photosPerPage int) string {
	base := filepath.Base(inputJSON)
	stem := strings.TrimSuffix(base, filepath.Ext(base))
	return fmt.Sprintf("%s_%dup.pdf", stem, photosPerPage)
}
