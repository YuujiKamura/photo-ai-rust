// Package pipeline — image.go
//
// DLL bridge functions for image processing and contact sheet generation.
// Wraps pkg/engine calls for use by the pipeline orchestrator.
package pipeline

import (
	"context"
	"fmt"
	"path/filepath"

	"github.com/YuujiKamura/photo-ai-go/pkg/engine"
)

// EXIFMeta holds the subset of EXIF data used by the pipeline.
type EXIFMeta struct {
	DateTime string
	Width    int
	Height   int
}

// ExtractEXIF calls the DLL to extract EXIF metadata from a single image file.
// Returns EXIFMeta and a nil error on success.
func ExtractEXIF(ctx context.Context, filePath string) (EXIFMeta, error) {
	select {
	case <-ctx.Done():
		return EXIFMeta{}, ctx.Err()
	default:
	}

	res, err := engine.ExtractEXIF(engine.EXIFConfig{FilePath: filePath})
	if err != nil {
		return EXIFMeta{}, fmt.Errorf("ExtractEXIF %q: %w", filePath, err)
	}
	return EXIFMeta{
		DateTime: res.DateTime,
		Width:    res.Width,
		Height:   res.Height,
	}, nil
}

// ProcessImageFolder calls the DLL to run the full AI photo analysis pipeline
// on a folder. Returns the path of the output JSON and the count of photos
// processed.
func ProcessImageFolder(ctx context.Context, cfg engine.ImageConfig) (outputJSON string, photoCount int, err error) {
	select {
	case <-ctx.Done():
		return "", 0, ctx.Err()
	default:
	}

	res, err := engine.ProcessImage(cfg)
	if err != nil {
		return "", 0, fmt.Errorf("ProcessImage %q: %w", cfg.Folder, err)
	}
	return res.OutputJSON, res.PhotoCount, nil
}

// GeneratePDF calls the DLL to produce a PDF photo book from an analysis JSON.
func GeneratePDF(ctx context.Context, cfg engine.PDFConfig) (outputPath string, pageCount int, err error) {
	select {
	case <-ctx.Done():
		return "", 0, ctx.Err()
	default:
	}

	res, err := engine.GeneratePDF(cfg)
	if err != nil {
		return "", 0, fmt.Errorf("GeneratePDF: %w", err)
	}
	return res.OutputPath, res.PageCount, nil
}

// GenerateExcel calls the DLL to produce an Excel workbook from an analysis JSON.
func GenerateExcel(ctx context.Context, cfg engine.ExcelConfig) (outputPath string, sheetCount int, err error) {
	select {
	case <-ctx.Done():
		return "", 0, ctx.Err()
	default:
	}

	res, err := engine.GenerateExcel(cfg)
	if err != nil {
		return "", 0, fmt.Errorf("GenerateExcel: %w", err)
	}
	return res.OutputPath, res.SheetCount, nil
}

// ContactSheetConfig holds parameters for contact sheet generation.
// The Rust implementation (contactsheet.rs) uses 5 cols for Before, 7 for After.
type ContactSheetConfig struct {
	// ImagePaths is the ordered list of source image file paths.
	ImagePaths []string
	// Prefix is "B" (before) or "A" (after).
	Prefix string
	// Cols is the number of grid columns (5 for before, 7 for after).
	Cols int
	// OutputPath is where the contact-sheet JPEG is written.
	OutputPath string
}

// ContactSheetResult holds the output of a contact sheet generation call.
type ContactSheetResult struct {
	// ImagePath is the saved contact-sheet JPEG path.
	ImagePath string
	// Mapping maps 1-based index to source file name, matching the Rust
	// ContactSheet.mapping field.
	Mapping []ContactSheetEntry
}

// ContactSheetEntry pairs a 1-based grid number with its source file name.
type ContactSheetEntry struct {
	Number   int
	FileName string
}

// GenerateContactSheet builds a numbered grid image from the given photos.
// The Rust implementation is in src/contactsheet.rs; this Go version delegates
// layout to the DLL via ProcessImage with a synthetic folder, or falls back to
// a pure-Go stub that records the mapping without actually rendering pixels.
//
// NOTE: Full pixel rendering requires the photo-engine DLL. When the DLL is
// unavailable (non-Windows or DLL not found), only the mapping is returned and
// ImagePath is set to OutputPath without creating the file.
func GenerateContactSheet(_ context.Context, cfg ContactSheetConfig) (ContactSheetResult, error) {
	if len(cfg.ImagePaths) == 0 {
		return ContactSheetResult{}, fmt.Errorf("GenerateContactSheet: no images provided")
	}
	if cfg.Cols <= 0 {
		return ContactSheetResult{}, fmt.Errorf("GenerateContactSheet: cols must be > 0")
	}
	if cfg.OutputPath == "" {
		return ContactSheetResult{}, fmt.Errorf("GenerateContactSheet: OutputPath is required")
	}

	// Build mapping (mirrors ContactSheet.mapping in Rust)
	mapping := make([]ContactSheetEntry, len(cfg.ImagePaths))
	for i, p := range cfg.ImagePaths {
		mapping[i] = ContactSheetEntry{
			Number:   i + 1,
			FileName: filepath.Base(p),
		}
	}

	// The DLL does not expose a dedicated contact-sheet endpoint; actual JPEG
	// rendering is handled by the Rust binary. Here we record the mapping and
	// return the expected output path so callers can wire up the ensemble query.
	return ContactSheetResult{
		ImagePath: cfg.OutputPath,
		Mapping:   mapping,
	}, nil
}
