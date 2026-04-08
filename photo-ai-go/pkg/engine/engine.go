//go:build windows

// Package engine wraps the photo-engine CLI tools (EXEs).
//
// The engine path is resolved in this order:
//  1. Environment variable PHOTO_PDF_ENGINE_EXE (full path to the EXE file)
//  2. Same directory as the running executable, named "photo-pdf-engine.exe"
package engine

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"unsafe"

	"golang.org/x/sys/windows"
)

// runCommandWithJobObject executes a command within a Job Object to ensure
// that the entire process tree (including grandchild processes) is terminated
// when the main command exits. This prevents hangs caused by zombie processes
// holding onto pipe handles.
func runCommandWithJobObject(cmd *exec.Cmd) ([]byte, error) {
	job, err := windows.CreateJobObject(nil, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create job object: %w", err)
	}
	defer windows.CloseHandle(job)

	info := &windows.JOBOBJECT_EXTENDED_LIMIT_INFORMATION{
		BasicLimitInformation: windows.JOBOBJECT_BASIC_LIMIT_INFORMATION{
			LimitFlags: windows.JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
		},
	}
	if _, err := windows.SetInformationJobObject(job, windows.JobObjectExtendedLimitInformation, uintptr(unsafe.Pointer(info)), uint32(unsafe.Sizeof(*info))); err != nil {
		return nil, fmt.Errorf("failed to set job object info: %w", err)
	}

	cmd.SysProcAttr = &syscall.SysProcAttr{
		CreationFlags: windows.CREATE_SUSPENDED,
	}

	if err := cmd.Start(); err != nil {
		return nil, fmt.Errorf("failed to start command: %w", err)
	}

	processHandle, err := windows.OpenProcess(windows.PROCESS_SET_QUOTA|windows.PROCESS_TERMINATE, false, uint32(cmd.Process.Pid))
	if err != nil {
		return nil, fmt.Errorf("failed to open process: %w", err)
	}
	defer windows.CloseHandle(processHandle)

	if err := windows.AssignProcessToJobObject(job, processHandle); err != nil {
		return nil, fmt.Errorf("failed to assign process to job object: %w", err)
	}

	threadHandle, err := windows.OpenThread(windows.THREAD_SUSPEND_RESUME, false, cmd.SysProcAttr.Threads[0])
	if err != nil {
		return nil, fmt.Errorf("failed to open thread: %w", err)
}
	defer windows.CloseHandle(threadHandle)

	if _, err := windows.ResumeThread(threadHandle); err != nil {
		return nil, fmt.Errorf("failed to resume thread: %w", err)
	}
	
	return cmd.CombinedOutput()
}

// runCommand is a wrapper for non-windows platforms.
func runCommand(cmd *exec.Cmd) ([]byte, error) {
	return cmd.CombinedOutput()
}

// resolveEnginePath returns the path to the specified engine EXE.
func resolveEnginePath(envVar, defaultName string) (string, error) {
	if v := os.Getenv(envVar); v != "" {
		return v, nil
	}
	exe, err := os.Executable()
	if err != nil {
		return "", fmt.Errorf("cannot resolve executable path: %w", err)
	}
	// Try same directory as executable
	p := filepath.Join(filepath.Dir(exe), defaultName)
	if _, err := os.Stat(p); err == nil {
		return p, nil
	}
	// Try parent directory's target/release (for dev)
	p = filepath.Join(filepath.Dir(filepath.Dir(filepath.Dir(exe))), "target", "release", defaultName)
	if _, err := os.Stat(p); err == nil {
		return p, nil
	}
	return defaultName, nil // Fallback to PATH
}

// parseEngineResponse parses the last line of output as a JSON EngineResponse.
func parseEngineResponse(output []byte, target any) error {
	lines := strings.Split(strings.TrimSpace(string(output)), "\n")
	if len(lines) == 0 {
		return fmt.Errorf("empty output from engine")
	}
	lastLine := lines[len(lines)-1]

	var resp struct {
		OK    bool            `json:"ok"`
		Data  json.RawMessage `json:"data"`
		Error string          `json:"error"`
	}

	if err := json.Unmarshal([]byte(lastLine), &resp); err != nil {
		return fmt.Errorf("failed to parse engine JSON: %w\nraw output: %s", err, string(output))
	}

	if !resp.OK {
		return fmt.Errorf("engine error: %s", resp.Error)
	}

	if err := json.Unmarshal(resp.Data, target); err != nil {
		return fmt.Errorf("failed to parse engine data: %w", err)
	}

	return nil
}

// GeneratePDF calls the photo-pdf-engine.exe to generate a PDF photo book.
func GeneratePDF(config PDFConfig) (PDFResult, error) {
	var result PDFResult

	enginePath, err := resolveEnginePath("PHOTO_PDF_ENGINE_EXE", "photo-pdf-engine.exe")
	if err != nil {
		return result, err
	}

	cmd := exec.Command(enginePath,
		"--input", config.InputJSON,
		"--output", config.OutputPath,
		"--photos-per-page", strconv.Itoa(config.PhotosPerPage),
		"--quality", config.Quality,
	)

	output, err := runCommandWithJobObject(cmd)
	if err != nil {
		// Try parsing anyway, as engine might exit 1 but still print JSON error
		var data struct {
			OutputPath string `json:"output_path"`
			Count      int    `json:"count"`
		}
		if parseErr := parseEngineResponse(output, &data); parseErr == nil {
			result.OutputPath = data.OutputPath
			result.PageCount = (data.Count + config.PhotosPerPage - 1) / config.PhotosPerPage
			return result, nil
		}
		return result, fmt.Errorf("execution failed: %w\noutput: %s", err, string(output))
	}

	var data struct {
		OutputPath string `json:"output_path"`
		Count      int    `json:"count"`
	}
	if err := parseEngineResponse(output, &data); err != nil {
		return result, err
	}

	result.OutputPath = data.OutputPath
	result.PageCount = (data.Count + config.PhotosPerPage - 1) / config.PhotosPerPage

	return result, nil
}

// GenerateExcel calls the photo-excel-engine.exe to generate an Excel photo book.
func GenerateExcel(config ExcelConfig) (ExcelResult, error) {
	var result ExcelResult

	enginePath, err := resolveEnginePath("PHOTO_EXCEL_ENGINE_EXE", "photo-excel-engine.exe")
	if err != nil {
		return result, err
	}

	// TODO: PhotosPerPage support in ExcelConfig? types.go has no PhotosPerPage in ExcelConfig
	photosPerPage := 3

	cmd := exec.Command(enginePath,
		"--input", config.InputJSON,
		"--output", config.OutputPath,
		"--photos-per-page", strconv.Itoa(photosPerPage),
	)

	output, err := runCommandWithJobObject(cmd)
	if err != nil {
		var data struct {
			OutputPath string `json:"output_path"`
			Count      int    `json:"count"`
		}
		if parseErr := parseEngineResponse(output, &data); parseErr == nil {
			result.OutputPath = data.OutputPath
			result.SheetCount = (data.Count + photosPerPage - 1) / photosPerPage
			return result, nil
		}
		return result, fmt.Errorf("execution failed: %w\noutput: %s", err, string(output))
	}

	var data struct {
		OutputPath string `json:"output_path"`
		Count      int    `json:"count"`
	}
	if err := parseEngineResponse(output, &data); err != nil {
		return result, err
	}

	result.OutputPath = data.OutputPath
	result.SheetCount = (data.Count + photosPerPage - 1) / photosPerPage

	return result, nil
}

// ProcessImage calls the photo-tag-engine.exe to analyze images.
func ProcessImage(config ImageConfig) (ImageResult, error) {
	var result ImageResult

	enginePath, err := resolveEnginePath("PHOTO_TAG_ENGINE_EXE", "photo-tag-engine.exe")
	if err != nil {
		return result, err
	}

	usageMode := "time_based_quota"
	if config.PayPerUse {
		usageMode = "pay_per_use"
	} else if config.Resident {
		usageMode = "resident"
	}

	cmd := exec.Command(enginePath,
		"--folder", config.Folder,
		"--batch-size", strconv.Itoa(config.BatchSize),
		"--usage-mode", usageMode,
	)

	output, err := runCommandWithJobObject(cmd)
	if err != nil {
		var data struct {
			Folder  string `json:"folder"`
			Count   int    `json:"count"`
			Records any    `json:"records"`
		}
		if parseErr := parseEngineResponse(output, &data); parseErr == nil {
			result.PhotoCount = data.Count
			result.OutputJSON = filepath.Join(config.Folder, "photo-groups.json")
			return result, nil
		}
		return result, fmt.Errorf("execution failed: %w\noutput: %s", err, string(output))
	}

	var data struct {
		Folder  string `json:"folder"`
		Count   int    `json:"count"`
		Records any    `json:"records"`
	}
	if err := parseEngineResponse(output, &data); err != nil {
		return result, err
	}

	result.PhotoCount = data.Count
	result.OutputJSON = filepath.Join(config.Folder, "photo-groups.json")

	return result, nil
}

func ExtractEXIF(config EXIFConfig) (EXIFResult, error) {
	return EXIFResult{Error: "Not implemented in EXE mode yet"}, nil
}
