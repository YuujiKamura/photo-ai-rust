//go:build !windows

// Package engine wraps the photo-engine shared library.
// On non-Windows platforms, dynamic loading is not yet implemented.
// Build with CGo LDFLAGS pointing to a compiled libphoto-engine.so/.dylib
// when targeting Linux or macOS.
package engine

import "errors"

var errNotImplemented = errors.New("photo-engine dynamic loading is not yet implemented on this platform; " +
	"set PHOTO_ENGINE_LIB and build with CGo LDFLAGS on Linux/macOS")

// Load is a no-op stub on non-Windows platforms.
func Load() error { return errNotImplemented }

// GeneratePDF is a stub on non-Windows platforms.
func GeneratePDF(_ PDFConfig) (PDFResult, error) { return PDFResult{}, errNotImplemented }

// GenerateExcel is a stub on non-Windows platforms.
func GenerateExcel(_ ExcelConfig) (ExcelResult, error) { return ExcelResult{}, errNotImplemented }

// ProcessImage is a stub on non-Windows platforms.
func ProcessImage(_ ImageConfig) (ImageResult, error) { return ImageResult{}, errNotImplemented }

// ExtractEXIF is a stub on non-Windows platforms.
func ExtractEXIF(_ EXIFConfig) (EXIFResult, error) { return EXIFResult{}, errNotImplemented }
