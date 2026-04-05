//go:build windows

// Package engine wraps the photo-engine shared library (DLL).
//
// The library path is resolved in this order:
//  1. Environment variable PHOTO_ENGINE_LIB (full path to the DLL file)
//  2. Same directory as the running executable, named "photo-engine.dll"
//
// The DLL uses a buffer-based ABI: the caller provides a byte buffer and the
// DLL writes its JSON response into it. Return value is bytes written (>=0)
// or negative required size if the buffer was too small.
//
// No CGo is used; all DLL interaction goes through syscall.
package engine

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"syscall"
	"unsafe"
)

// lib holds the loaded DLL handle and resolved proc pointers.
var lib *photoEngineLib

// photoEngineLib holds Windows DLL proc handles.
type photoEngineLib struct {
	genPDF    *syscall.Proc
	genExcel  *syscall.Proc
	procImage *syscall.Proc
	extEXIF   *syscall.Proc
}

// resolvePath returns the path to the DLL, preferring PHOTO_ENGINE_LIB, then
// the executable's directory.
func resolvePath() (string, error) {
	if v := os.Getenv("PHOTO_ENGINE_LIB"); v != "" {
		return v, nil
	}
	exe, err := os.Executable()
	if err != nil {
		return "", fmt.Errorf("cannot resolve executable path: %w", err)
	}
	return filepath.Join(filepath.Dir(exe), "photo-engine.dll"), nil
}

// Load explicitly loads the DLL. Calling it is optional; each exported function
// calls it lazily on first use.
func Load() error {
	if lib != nil {
		return nil
	}
	path, err := resolvePath()
	if err != nil {
		return err
	}
	dll, err := syscall.LoadDLL(path)
	if err != nil {
		return fmt.Errorf("LoadDLL %q: %w", path, err)
	}
	l := &photoEngineLib{}
	type entry struct {
		name string
		ptr  **syscall.Proc
	}
	entries := []entry{
		{"photo_engine_generate_pdf", &l.genPDF},
		{"photo_engine_generate_excel", &l.genExcel},
		{"photo_engine_process_image", &l.procImage},
		{"photo_engine_extract_exif", &l.extEXIF},
	}
	for _, e := range entries {
		p, findErr := dll.FindProc(e.name)
		if findErr != nil {
			return fmt.Errorf("FindProc %q: %w", e.name, findErr)
		}
		*e.ptr = p
	}
	lib = l
	return nil
}

// defaultBufSize is the initial response buffer size (64 KiB).
const defaultBufSize = 64 * 1024

// callDLL marshals req to JSON, calls the DLL proc with a buffer-based ABI:
//
//	int32_t proc(const char* req_json, char* out_buf, size_t out_len)
//
// Returns bytes written on success, or negative required size if buffer too small.
func callDLL(proc *syscall.Proc, req interface{}, resp interface{}) error {
	if err := Load(); err != nil {
		return err
	}
	reqJSON, err := json.Marshal(req)
	if err != nil {
		return fmt.Errorf("marshal request: %w", err)
	}
	// Null-terminate the request JSON.
	reqBytes := append(reqJSON, 0)

	// Allocate response buffer.
	bufSize := defaultBufSize
	outBuf := make([]byte, bufSize)

	ret, _, _ := proc.Call(
		uintptr(unsafe.Pointer(&reqBytes[0])),
		uintptr(unsafe.Pointer(&outBuf[0])),
		uintptr(bufSize),
	)

	written := int32(ret)

	// Negative return means buffer too small; absolute value is required size.
	if written < 0 {
		needed := int(-written)
		if needed > 10*1024*1024 {
			return fmt.Errorf("DLL requested unreasonable buffer size: %d", needed)
		}
		outBuf = make([]byte, needed)
		ret, _, _ = proc.Call(
			uintptr(unsafe.Pointer(&reqBytes[0])),
			uintptr(unsafe.Pointer(&outBuf[0])),
			uintptr(needed),
		)
		written = int32(ret)
		if written < 0 {
			return fmt.Errorf("DLL retry failed, returned %d", written)
		}
	}

	if written == 0 {
		return errors.New("DLL function returned empty response")
	}

	if err := json.Unmarshal(outBuf[:written], resp); err != nil {
		return fmt.Errorf("unmarshal response: %w", err)
	}
	return nil
}

// GeneratePDF calls the DLL to generate a PDF photo book.
func GeneratePDF(config PDFConfig) (PDFResult, error) {
	var result PDFResult
	if err := callDLL(lib.genPDF, config, &result); err != nil {
		return result, err
	}
	if result.Error != "" {
		return result, errors.New(result.Error)
	}
	return result, nil
}

// GenerateExcel calls the DLL to generate an Excel workbook.
func GenerateExcel(config ExcelConfig) (ExcelResult, error) {
	var result ExcelResult
	if err := callDLL(lib.genExcel, config, &result); err != nil {
		return result, err
	}
	if result.Error != "" {
		return result, errors.New(result.Error)
	}
	return result, nil
}

// ProcessImage calls the DLL to run the AI photo analysis pipeline.
func ProcessImage(config ImageConfig) (ImageResult, error) {
	var result ImageResult
	if err := callDLL(lib.procImage, config, &result); err != nil {
		return result, err
	}
	if result.Error != "" {
		return result, errors.New(result.Error)
	}
	return result, nil
}

// ExtractEXIF calls the DLL to extract EXIF metadata from an image file.
func ExtractEXIF(config EXIFConfig) (EXIFResult, error) {
	var result EXIFResult
	if err := callDLL(lib.extEXIF, config, &result); err != nil {
		return result, err
	}
	if result.Error != "" {
		return result, errors.New(result.Error)
	}
	return result, nil
}
