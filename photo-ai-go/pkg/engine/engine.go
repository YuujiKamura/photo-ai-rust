//go:build windows

// Package engine wraps the photo-engine CLI tools (EXEs).
//
// The engine path is resolved in this order:
//  1. Environment variable PHOTO_PDF_ENGINE_EXE (full path to the EXE file)
//  2. Same directory as the running executable, named "photo-pdf-engine.exe"
package engine

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"unsafe"

	"github.com/YuujiKamura/photo-ai-go/internal/engines"
	"golang.org/x/sys/windows"
)

var GitCommit = "unknown"

var (
	ensureOnce sync.Once
	ensureDir  string
	ensureErr  error
)

// EnsureEngines extracts embedded engine binaries to a temporary directory.
// It is safe to call concurrently; extraction happens exactly once.
func EnsureEngines() (string, error) {
	ensureOnce.Do(func() {
		ensureDir, ensureErr = doEnsureEngines()
	})
	return ensureDir, ensureErr
}

func doEnsureEngines() (string, error) {
	tempDir := filepath.Join(os.TempDir(), "photo-ai", "bin", GitCommit)
	if err := os.MkdirAll(tempDir, 0755); err != nil {
		return "", fmt.Errorf("failed to create temp dir: %w", err)
	}

	engineNames := []string{
		"photo-tag-engine.exe",
		"photo-analysis-engine.exe",
		"photo-pdf-engine.exe",
		"photo-excel-engine.exe",
	}

	for _, name := range engineNames {
		targetPath := filepath.Join(tempDir, name)
		data, err := engines.EmbeddedEngines.ReadFile(name)
		if err != nil {
			// If not in embedded FS, skip (might be in dev mode)
			continue
		}

		if matchesEmbeddedFile(targetPath, data) {
			continue
		}

		if err := os.WriteFile(targetPath, data, 0755); err != nil {
			return "", fmt.Errorf("failed to extract %s: %w", name, err)
		}
	}

	return tempDir, nil
}

func matchesEmbeddedFile(targetPath string, expected []byte) bool {
	current, err := os.ReadFile(targetPath)
	if err != nil {
		return false
	}
	if len(current) != len(expected) {
		return false
	}
	return bytes.Equal(current, expected)
}

// runCommandWithJobObject executes a command within a Job Object to ensure
// that the entire process tree (including grandchild processes) is terminated
// when the main command exits. This prevents hangs caused by zombie processes
// holding onto pipe handles.
func runCommandWithJobObject(cmd *exec.Cmd) ([]byte, error) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr

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

	cmd.SysProcAttr = &syscall.SysProcAttr{}

	if err := cmd.Start(); err != nil {
		return nil, fmt.Errorf("failed to start command: %w", err)
	}

	processHandle, err := windows.OpenProcess(windows.PROCESS_SET_QUOTA|windows.PROCESS_TERMINATE, false, uint32(cmd.Process.Pid))
	if err != nil {
		return nil, fmt.Errorf("failed to open process: %w", err)
	}
	defer windows.CloseHandle(processHandle)

	if err := windows.AssignProcessToJobObject(job, processHandle); err != nil {
		if waitErr := cmd.Wait(); waitErr != nil {
			return append(stdout.Bytes(), stderr.Bytes()...), fmt.Errorf("command failed: %w", waitErr)
		}
		return append(stdout.Bytes(), stderr.Bytes()...), nil
	}

	if err := cmd.Wait(); err != nil {
		return append(stdout.Bytes(), stderr.Bytes()...), fmt.Errorf("command failed: %w", err)
	}

	return append(stdout.Bytes(), stderr.Bytes()...), nil
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

	// 1. Try extracted temp directory
	if extractedDir, err := EnsureEngines(); err == nil {
		p := filepath.Join(extractedDir, defaultName)
		if _, err := os.Stat(p); err == nil {
			return p, nil
		}
	}

	exe, err := os.Executable()
	if err != nil {
		return "", fmt.Errorf("cannot resolve executable path: %w", err)
	}
	// 2. Try same directory as executable
	p := filepath.Join(filepath.Dir(exe), defaultName)
	if _, err := os.Stat(p); err == nil {
		return p, nil
	}
	// 3. Try parent directory's target/release (for dev)
	p = filepath.Join(filepath.Dir(filepath.Dir(filepath.Dir(exe))), "target", "release", defaultName)
	if _, err := os.Stat(p); err == nil {
		return p, nil
	}
	return defaultName, nil // Fallback to PATH
}

// parseEngineResponse parses the last JSON line of output as an EngineResponse.
func parseEngineResponse(output []byte, target any) error {
	lines := strings.Split(strings.TrimSpace(string(output)), "\n")
	if len(lines) == 0 {
		return fmt.Errorf("empty output from engine")
	}

	var (
		resp     struct {
			OK    bool            `json:"ok"`
			Data  json.RawMessage `json:"data"`
			Error string          `json:"error"`
		}
		parseErr error
	)

	for i := len(lines) - 1; i >= 0; i-- {
		line := strings.TrimSpace(lines[i])
		if line == "" || !(strings.HasPrefix(line, "{") && strings.HasSuffix(line, "}")) {
			continue
		}
		if err := json.Unmarshal([]byte(line), &resp); err == nil {
			if !resp.OK {
				return fmt.Errorf("engine error: %s", resp.Error)
			}
			if err := json.Unmarshal(resp.Data, target); err != nil {
				return fmt.Errorf("failed to parse engine data: %w", err)
			}
			return nil
		} else {
			parseErr = err
		}
	}

	if parseErr != nil {
		return fmt.Errorf("failed to parse engine JSON: %w\nraw output: %s", parseErr, string(output))
	}
	return fmt.Errorf("failed to find engine JSON in output\nraw output: %s", string(output))
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

// MatchMaster calls the photo-analysis-engine.exe to match analysis results with a master CSV.
func MatchMaster(inputJSON, masterPath, folderPath, lineTypes, folderRules string) ([]AnalysisResult, error) {
	enginePath, err := resolveEnginePath("PHOTO_ANALYSIS_ENGINE_EXE", "photo-analysis-engine.exe")
	if err != nil {
		return nil, err
	}

	args := []string{"match-master",
		"--input", inputJSON,
		"--master", masterPath,
		"--folder", folderPath,
	}
	if lineTypes != "" {
		args = append(args, "--line-types", lineTypes)
	}
	if folderRules != "" {
		args = append(args, "--folder-rules", folderRules)
	}

	cmd := exec.Command(enginePath, args...)

	output, err := runCommandWithJobObject(cmd)
	if err != nil {
		return nil, fmt.Errorf("match-master failed: %w\noutput: %s", err, string(output))
	}

	var results []AnalysisResult
	if err := parseEngineResponse(output, &results); err != nil {
		return nil, err
	}

	return results, nil
}

// Normalize calls the photo-analysis-engine.exe to normalize analysis results.
func Normalize(inputJSON, folderPath, station, folderRules string) ([]AnalysisResult, error) {
	enginePath, err := resolveEnginePath("PHOTO_ANALYSIS_ENGINE_EXE", "photo-analysis-engine.exe")
	if err != nil {
		return nil, err
	}

	args := []string{"normalize",
		"--input", inputJSON,
		"--folder", folderPath,
	}
	if station != "" {
		args = append(args, "--station", station)
	}
	if folderRules != "" {
		args = append(args, "--folder-rules", folderRules)
	}

	cmd := exec.Command(enginePath, args...)

	output, err := runCommandWithJobObject(cmd)
	if err != nil {
		return nil, fmt.Errorf("normalize failed: %w\noutput: %s", err, string(output))
	}

	var results []AnalysisResult
	if err := parseEngineResponse(output, &results); err != nil {
		return nil, err
	}

	return results, nil
}

func ExtractEXIF(config EXIFConfig) (EXIFResult, error) {
	return EXIFResult{Error: "Not implemented in EXE mode yet"}, nil
}
