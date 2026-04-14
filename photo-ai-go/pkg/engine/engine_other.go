//go:build !windows

// Package engine wraps the photo-engine shared library.
// On non-Windows platforms, dynamic loading is not yet implemented.
// Build with CGo LDFLAGS pointing to a compiled libphoto-engine.so/.dylib
// when targeting Linux or macOS.
package engine

import (
	"context"
	"errors"
	"fmt"

	"github.com/YuujiKamura/photo-ai-go/pkg/tagger"
)

var errNotImplemented = errors.New("photo-engine dynamic loading is not yet implemented on this platform; " +
	"set PHOTO_ENGINE_LIB and build with CGo LDFLAGS on Linux/macOS")

var runGrouping = tagger.RunGrouping

// Load is a no-op stub on non-Windows platforms.
func Load() error { return errNotImplemented }

// GeneratePDF is a stub on non-Windows platforms.
func GeneratePDF(_ PDFConfig) (PDFResult, error) { return PDFResult{}, errNotImplemented }

// GenerateExcel is a stub on non-Windows platforms.
func GenerateExcel(_ ExcelConfig) (ExcelResult, error) { return ExcelResult{}, errNotImplemented }

// ProcessImage classifies and groups construction photos using Gemini AI (pure Go).
func ProcessImage(config ImageConfig) (ImageResult, error) {
	cfg := tagger.Config{
		Folder:    config.Folder,
		BatchSize: config.BatchSize,
		PayPerUse: config.PayPerUse,
	}
	if cfg.BatchSize <= 0 {
		cfg.BatchSize = 10
	}
	res, err := runGrouping(context.Background(), cfg)
	if err != nil {
		return ImageResult{}, fmt.Errorf("tagger.RunGrouping: %w", err)
	}
	return ImageResult{PhotoCount: res.PhotoCount, OutputJSON: res.OutputJSON}, nil
}

// ExtractEXIF is a stub on non-Windows platforms.
func ExtractEXIF(_ EXIFConfig) (EXIFResult, error) { return EXIFResult{}, errNotImplemented }

// MatchMaster is a stub on non-Windows platforms.
func MatchMaster(_, _, _, _, _ string) ([]AnalysisResult, error) {
	return nil, errNotImplemented
}

// Normalize is a stub on non-Windows platforms.
func Normalize(_, _, _, _ string) ([]AnalysisResult, error) {
	return nil, errNotImplemented
}
