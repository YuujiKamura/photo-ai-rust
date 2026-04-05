package export

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/YuujiKamura/photo-ai-go/pkg/engine"
)

// GenerateExcel builds an ExcelConfig from analysis results and calls the engine DLL.
//
// Parameters:
//   - inputJSON:  path to the result.json file (passed directly to the DLL)
//   - outputPath: desired output path; if empty or a directory, a filename is derived
//   - preset:     alias preset name, e.g. "pavement" (may be empty)
//   - aliasFile:  path to a custom alias JSON file (may be empty)
func GenerateExcel(inputJSON, outputPath, preset, aliasFile string) (engine.ExcelResult, error) {
	defaultName := deriveExcelName(inputJSON)
	resolvedOutput := ResolveExportPath(inputJSON, outputPath, defaultName)

	cfg := engine.ExcelConfig{
		InputJSON:  inputJSON,
		OutputPath: resolvedOutput,
		Preset:     preset,
		AliasFile:  aliasFile,
	}

	result, err := engine.GenerateExcel(cfg)
	if err != nil {
		return result, fmt.Errorf("GenerateExcel: %w", err)
	}

	return result, nil
}

// deriveExcelName builds a default output Excel filename from the input JSON path.
// e.g. "result.json" → "result.xlsx"
func deriveExcelName(inputJSON string) string {
	base := filepath.Base(inputJSON)
	stem := strings.TrimSuffix(base, filepath.Ext(base))
	return stem + ".xlsx"
}
