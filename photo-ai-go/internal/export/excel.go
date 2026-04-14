package export

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/YuujiKamura/photo-ai-go/pkg/engine"
)

// GenerateExcel builds an ExcelConfig from analysis results and dispatches to
// the appropriate backend based on the PHOTO_EXCEL_BACKEND environment variable.
//
// Backend selection:
//   - "" or "dll"  → existing DLL/subprocess path (default, unchanged)
//   - "go"         → pure-Go excelize path (excel_native.go)
//   - any other    → error with a clear message listing valid values
//
// Parameters:
//   - inputJSON:  path to the result.json file (passed directly to the DLL)
//   - outputPath: desired output path; if empty or a directory, a filename is derived
//   - preset:     alias preset name, e.g. "pavement" (may be empty)
//   - aliasFile:  path to a custom alias JSON file (may be empty)
func GenerateExcel(inputJSON, outputPath, preset, aliasFile string) (engine.ExcelResult, error) {
	backend := strings.ToLower(strings.TrimSpace(os.Getenv("PHOTO_EXCEL_BACKEND")))

	switch backend {
	case "", "dll":
		return generateExcelDLL(inputJSON, outputPath, preset, aliasFile)
	case "go":
		return generateExcelGo(inputJSON, outputPath)
	default:
		return engine.ExcelResult{}, fmt.Errorf(
			"PHOTO_EXCEL_BACKEND=%q is not a valid value; use \"\" (default), \"dll\", or \"go\"",
			backend,
		)
	}
}

// generateExcelDLL is the original DLL/subprocess backend. Behavior is unchanged.
func generateExcelDLL(inputJSON, outputPath, preset, aliasFile string) (engine.ExcelResult, error) {
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
		return result, fmt.Errorf("GenerateExcel(dll): %w", err)
	}

	return result, nil
}

// generateExcelGo is the pure-Go excelize backend.
func generateExcelGo(inputJSON, outputPath string) (engine.ExcelResult, error) {
	defaultName := deriveExcelName(inputJSON)
	resolvedOutput := ResolveExportPath(inputJSON, outputPath, defaultName)

	result, err := GenerateExcelNative(inputJSON, resolvedOutput, 3)
	if err != nil {
		return result, fmt.Errorf("GenerateExcel(go): %w", err)
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
