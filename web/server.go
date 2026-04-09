package main

import (
	"bufio"
	"encoding/base64"
	"encoding/csv"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log"
	"mime"
	"net/http"
	"net/http/httputil"
	"net/url"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"regexp"
	"strings"
	"sync"
	"syscall"
	"time"

	"github.com/YuujiKamura/deckpilot/daemon"
	"golang.org/x/net/websocket"
)

type MasterRow struct {
	Division       string `json:"division"`
	PhotoType      string `json:"photoType"`
	WorkType       string `json:"workType"`
	Variety        string `json:"variety"`
	Subphase       string `json:"subphase"`
	Remarks        string `json:"remarks"`
	SearchPatterns string `json:"searchPatterns"`
}

type JobState struct {
	Running      bool   `json:"running"`
	Action       string `json:"action,omitempty"`
	Folder       string `json:"folder,omitempty"`
	MasterPath   string `json:"masterPath,omitempty"`
	ResultPath   string `json:"resultPath,omitempty"`
	PdfPath      string `json:"pdfPath,omitempty"`
	ExcelPath    string `json:"excelPath,omitempty"`
	LastExitCode int    `json:"lastExitCode"`
	LastError    string `json:"lastError,omitempty"`
	LastStdout   string `json:"lastStdout,omitempty"`
	LastStderr   string `json:"lastStderr,omitempty"`
	UpdatedAt    string `json:"updatedAt,omitempty"`
}

type AppState struct {
	mu  sync.Mutex
	job JobState
}

type RuntimeStatus struct {
	WebReady              bool   `json:"webReady"`
	MainCLIPath           string `json:"mainCliPath,omitempty"`
	MainCLIAvailable      bool   `json:"mainCliAvailable"`
	TagEnginePath         string `json:"tagEnginePath,omitempty"`
	TagEngineAvailable    bool   `json:"tagEngineAvailable"`
	AnalysisEnginePath    string `json:"analysisEnginePath,omitempty"`
	AnalysisEnginePresent bool   `json:"analysisEngineAvailable"`
	PDFEnginePath         string `json:"pdfEnginePath,omitempty"`
	PDFEngineAvailable    bool   `json:"pdfEngineAvailable"`
	ExcelEnginePath       string `json:"excelEnginePath,omitempty"`
	ExcelEnginePresent    bool   `json:"excelEngineAvailable"`
	AgentTerminalMode     string `json:"agentTerminalMode"`
	AgentOptional         bool   `json:"agentOptional"`
}

var appState AppState
var (
	findMainCLIFunc       = findMainCLI
	runCLIFunc            = runCLI
	prepareMasterFileFunc = prepareMasterFile
)

func (s *AppState) snapshot() JobState {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.job
}

func (s *AppState) update(mutator func(*JobState)) JobState {
	s.mu.Lock()
	defer s.mu.Unlock()
	mutator(&s.job)
	s.job.UpdatedAt = time.Now().Format(time.RFC3339)
	return s.job
}

func loadMasterCSVs(repoDir string) (map[string][]MasterRow, error) {
	masterDir := filepath.Join(repoDir, "master", "by_work_type")
	entries, err := os.ReadDir(masterDir)
	if err != nil {
		return nil, err
	}
	result := make(map[string][]MasterRow)
	for _, e := range entries {
		if e.IsDir() || !strings.HasSuffix(e.Name(), ".csv") {
			continue
		}
		name := strings.TrimSuffix(e.Name(), ".csv")
		f, err := os.Open(filepath.Join(masterDir, e.Name()))
		if err != nil {
			continue
		}
		r := csv.NewReader(f)
		header, err := r.Read()
		if err != nil {
			f.Close()
			continue
		}
		// Strip BOM from first header
		if len(header) > 0 {
			header[0] = strings.TrimPrefix(header[0], "\ufeff")
		}
		colIdx := make(map[string]int)
		for i, h := range header {
			colIdx[h] = i
		}
		var rows []MasterRow
		for {
			record, err := r.Read()
			if err != nil {
				break
			}
			get := func(key string) string {
				if idx, ok := colIdx[key]; ok && idx < len(record) {
					return record[idx]
				}
				return ""
			}
			rows = append(rows, MasterRow{
				Division:       get("費目"),
				PhotoType:      get("写真区分"),
				WorkType:       get("工種"),
				Variety:        get("種別"),
				Subphase:       get("細別"),
				Remarks:        get("備考"),
				SearchPatterns: get("検索パターン"),
			})
		}
		f.Close()
		result[name] = rows
	}
	return result, nil
}

// getGhosttyPort returns the ghostty-web demo server port from env or default.
func getGhosttyPort() string {
	if p := os.Getenv("GHOSTTY_PORT"); p != "" {
		return p
	}
	return "8888"
}

// startGhosttyWeb starts the ghostty-web demo server as a child process.
// It auto-installs npm dependencies if needed.
// Returns the process (nil if failed) and a cleanup function.
func startGhosttyWeb(webDir string) (*exec.Cmd, func()) {
	ghosttyPort := getGhosttyPort()
	demoDir := filepath.Join(webDir, "ghostty-web", "demo")

	// Check if demo directory exists (submodule present)
	if _, err := os.Stat(filepath.Join(demoDir, "bin", "demo.js")); err != nil {
		log.Printf("ghostty-web submodule not found at %s, skipping", demoDir)
		return nil, func() {}
	}

	// Auto-install npm dependencies if needed
	nodeModules := filepath.Join(demoDir, "node_modules")
	if _, err := os.Stat(nodeModules); err != nil {
		log.Printf("Installing ghostty-web demo dependencies...")
		install := exec.Command("npm", "install")
		install.Dir = demoDir
		install.Stdout = os.Stdout
		install.Stderr = os.Stderr
		if err := install.Run(); err != nil {
			log.Printf("npm install failed: %v (ghostty-web terminal will be unavailable)", err)
			return nil, func() {}
		}
	}

	// Use bundled node.exe if present, otherwise fall back to system node
	nodeCmd := "node"
	bundledNode := filepath.Join(webDir, "node.exe")
	if _, err := os.Stat(bundledNode); err == nil {
		nodeCmd = bundledNode
	}

	// Start demo server
	cmd := exec.Command(nodeCmd, "bin/demo.js")
	cmd.Dir = demoDir
	cmd.Env = append(os.Environ(), "PORT="+ghosttyPort)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	if err := cmd.Start(); err != nil {
		log.Printf("Failed to start ghostty-web: %v", err)
		return nil, func() {}
	}
	log.Printf("ghostty-web demo server started on port %s (PID %d)", ghosttyPort, cmd.Process.Pid)

	cleanup := func() {
		if cmd.Process != nil {
			log.Printf("Stopping ghostty-web (PID %d)...", cmd.Process.Pid)
			cmd.Process.Kill()
			cmd.Wait()
		}
	}
	return cmd, cleanup
}

func main() {
	// Start deckpilot daemon in background
	go func() {
		log.Printf("Starting deckpilot daemon...")
		d := daemon.New()
		if err := d.Run(); err != nil {
			log.Printf("deckpilot daemon error: %v", err)
		}
	}()

	// Serve static files from the directory where server.go lives
	exe, err := os.Executable()
	if err != nil {
		// fallback: use current working directory
		exe, _ = os.Getwd()
		exe = filepath.Join(exe, "dummy")
	}
	webDir := filepath.Dir(exe)
	// If running via "go run", use the source file's directory instead
	if len(os.Args) > 0 {
		src := os.Args[0]
		if abs, err := filepath.Abs(src); err == nil {
			candidate := filepath.Dir(abs)
			if _, err := os.Stat(filepath.Join(candidate, "index.html")); err == nil {
				webDir = candidate
			}
		}
	}
	// Also check current working directory
	if cwd, err := os.Getwd(); err == nil {
		if _, err := os.Stat(filepath.Join(cwd, "index.html")); err == nil {
			webDir = cwd
		}
	}

	// Resolve repo root (web/ is inside the repo)
	repoDir := filepath.Dir(webDir)
	log.Printf("Serving static files from: %s", webDir)
	log.Printf("Repo root: %s", repoDir)

	// Start ghostty-web demo server as child process
	ghosttyCmd, ghosttyCleanup := startGhosttyWeb(webDir)
	defer ghosttyCleanup()
	_ = ghosttyCmd

	// Handle graceful shutdown to kill child processes
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)
	go func() {
		<-sigCh
		log.Printf("Shutting down...")
		ghosttyCleanup()
		os.Exit(0)
	}()

	// CORS Middleware
	cors := func(h http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.Header().Set("Access-Control-Allow-Origin", "*")
			w.Header().Set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
			w.Header().Set("Access-Control-Allow-Headers", "Content-Type")
			// NOTE: COOP/COEP removed — they block cross-origin WASM import from ghostty-web (port 8888).
			// ghostty-web works without SharedArrayBuffer in single-threaded mode.
			if r.Method == "OPTIONS" {
				return
			}
			h.ServeHTTP(w, r)
		})
	}

	mux := http.NewServeMux()
	mux.Handle("/ws/terminal", websocket.Handler(handleTerminal))
	mux.HandleFunc("/api/master", func(w http.ResponseWriter, r *http.Request) {
		data, err := loadMasterCSVs(repoDir)
		if err != nil {
			http.Error(w, err.Error(), 500)
			return
		}
		w.Header().Set("Content-Type", "application/json; charset=utf-8")
		json.NewEncoder(w).Encode(data)
	})
	mux.HandleFunc("/api/master/update", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			http.Error(w, "POST only", 405)
			return
		}
		var req struct {
			WorkTypeName string    `json:"workTypeName"`
			RowIndex     int       `json:"rowIndex"`
			Row          MasterRow `json:"row"`
		}
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, err.Error(), 400)
			return
		}
		csvPath := filepath.Join(repoDir, "master", "by_work_type", req.WorkTypeName+".csv")
		if err := updateCSVRow(csvPath, req.RowIndex, req.Row); err != nil {
			http.Error(w, err.Error(), 500)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{"status": "ok"})
	})
	mux.HandleFunc("/api/master/rename", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			http.Error(w, "POST only", 405)
			return
		}
		var req struct {
			OldName string `json:"oldName"`
			NewName string `json:"newName"`
		}
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, err.Error(), 400)
			return
		}
		if err := renameMasterFile(repoDir, req.OldName, req.NewName); err != nil {
			http.Error(w, err.Error(), 500)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{"status": "ok", "oldName": req.OldName, "newName": req.NewName})
	})
	mux.HandleFunc("/api/job", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "GET" {
			http.Error(w, "GET only", 405)
			return
		}
		w.Header().Set("Content-Type", "application/json; charset=utf-8")
		json.NewEncoder(w).Encode(appState.snapshot())
	})
	mux.HandleFunc("/api/result", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "GET" {
			http.Error(w, "GET only", 405)
			return
		}
		jsonPath := r.URL.Query().Get("path")
		if jsonPath == "" {
			http.Error(w, "missing ?path= parameter", 400)
			return
		}
		resolved := resolvePath(repoDir, jsonPath)
		if !isPathAllowed(repoDir, resolved) {
			http.Error(w, "path not allowed", 403)
			return
		}
		data, err := os.ReadFile(resolved)
		if err != nil {
			http.Error(w, fmt.Sprintf("failed to read %s: %v", resolved, err), 404)
			return
		}
		w.Header().Set("Content-Type", "application/json; charset=utf-8")
		w.Write(data)
	})

	mux.HandleFunc("/api/result/update", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			http.Error(w, "POST only", 405)
			return
		}
		var req struct {
			Path    string `json:"path"`
			Updates []struct {
				FileName string `json:"fileName"`
				Field    string `json:"field"`
				Value    string `json:"value"`
			} `json:"updates"`
		}
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, err.Error(), 400)
			return
		}
		if req.Path == "" {
			http.Error(w, "path is required", 400)
			return
		}
		resolved := resolvePath(repoDir, req.Path)
		if !isPathAllowed(repoDir, resolved) {
			http.Error(w, "path not allowed", 403)
			return
		}
		raw, err := os.ReadFile(resolved)
		if err != nil {
			http.Error(w, fmt.Sprintf("failed to read %s: %v", resolved, err), 404)
			return
		}
		var data []map[string]interface{}
		if err := json.Unmarshal(raw, &data); err != nil {
			http.Error(w, fmt.Sprintf("failed to parse JSON: %v", err), 400)
			return
		}
		updated := 0
		for _, u := range req.Updates {
			for _, record := range data {
				if fn, ok := record["fileName"].(string); ok && fn == u.FileName {
					record[u.Field] = u.Value
					updated++
					break
				}
			}
		}
		out, err := json.MarshalIndent(data, "", "  ")
		if err != nil {
			http.Error(w, fmt.Sprintf("failed to marshal JSON: %v", err), 500)
			return
		}
		if err := os.WriteFile(resolved, out, 0644); err != nil {
			http.Error(w, fmt.Sprintf("failed to write %s: %v", resolved, err), 500)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]interface{}{"status": "ok", "updated": updated})
	})

	mux.HandleFunc("/api/analyze", func(w http.ResponseWriter, r *http.Request) {
		handleAnalyze(w, r, repoDir)
	})

	mux.HandleFunc("/api/export/pdf", func(w http.ResponseWriter, r *http.Request) {
		handleExport(w, r, repoDir, "pdf")
	})

	mux.HandleFunc("/api/export/excel", func(w http.ResponseWriter, r *http.Request) {
		handleExport(w, r, repoDir, "excel")
	})

	mux.HandleFunc("/api/download", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "GET" {
			http.Error(w, "GET only", 405)
			return
		}
		filePath := r.URL.Query().Get("path")
		if filePath == "" {
			http.Error(w, "missing ?path= parameter", 400)
			return
		}
		resolved := resolvePath(repoDir, filePath)
		if !isPathAllowed(repoDir, resolved) {
			http.Error(w, "path traversal denied", 403)
			return
		}
		absResolved, err := filepath.Abs(resolved)
		if err != nil {
			http.Error(w, "invalid path", 400)
			return
		}

		info, err := os.Stat(absResolved)
		if err != nil {
			http.Error(w, fmt.Sprintf("file not found: %v", err), 404)
			return
		}
		if info.IsDir() {
			http.Error(w, "cannot download a directory", 400)
			return
		}

		fileName := filepath.Base(absResolved)
		contentType := mime.TypeByExtension(filepath.Ext(fileName))
		if contentType == "" {
			contentType = "application/octet-stream"
		}

		w.Header().Set("Content-Type", contentType)
		w.Header().Set("Content-Disposition", fmt.Sprintf("attachment; filename=\"%s\"", fileName))
		http.ServeFile(w, r, absResolved)
	})
	mux.HandleFunc("/api/file", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "GET" {
			http.Error(w, "GET only", 405)
			return
		}
		filePath := r.URL.Query().Get("path")
		if filePath == "" {
			http.Error(w, "missing ?path= parameter", 400)
			return
		}
		resolved := resolvePath(repoDir, filePath)
		if !isPathAllowed(repoDir, resolved) {
			http.Error(w, "path traversal denied", 403)
			return
		}
		absResolved, err := filepath.Abs(resolved)
		if err != nil {
			http.Error(w, "invalid path", 400)
			return
		}
		info, err := os.Stat(absResolved)
		if err != nil {
			http.Error(w, fmt.Sprintf("file not found: %v", err), 404)
			return
		}
		if info.IsDir() {
			http.Error(w, "cannot read a directory", 400)
			return
		}
		contentType := mime.TypeByExtension(filepath.Ext(absResolved))
		if contentType == "" {
			contentType = "application/octet-stream"
		}
		w.Header().Set("Content-Type", contentType)
		http.ServeFile(w, r, absResolved)
	})

	mux.HandleFunc("/api/merge-master", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			http.Error(w, "POST only", 405)
			return
		}
		var req struct {
			MasterFiles []string `json:"masterFiles"`
		}
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, err.Error(), 400)
			return
		}
		cleanup, masterPath, err := prepareMasterFileFunc(repoDir, req.MasterFiles)
		if err != nil {
			http.Error(w, err.Error(), 500)
			return
		}
		// Don't cleanup - the caller needs the file. It will be overwritten next time.
		_ = cleanup
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{"path": masterPath})
	})
	mux.HandleFunc("/api/browse", func(w http.ResponseWriter, r *http.Request) {
		// Open native folder picker via PowerShell
		cmd := exec.Command("powershell", "-NoProfile", "-Sta", "-Command",
			`[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Windows.Forms
$d = New-Object System.Windows.Forms.OpenFileDialog
$d.ValidateNames = $false
$d.CheckFileExists = $false
$d.CheckPathExists = $true
$d.FileName = 'フォルダを選択'
$d.Title = '写真フォルダを選択'
if ($d.ShowDialog() -eq 'OK') {
  [Console]::WriteLine([System.IO.Path]::GetDirectoryName($d.FileName))
} else {
  [Console]::WriteLine('')
}`)
		out, err := cmd.Output()
		if err != nil {
			http.Error(w, err.Error(), 500)
			return
		}
		path := strings.TrimSpace(string(out))
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{"path": path})
	})
	mux.HandleFunc("/api/cp/send", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			http.Error(w, "POST only", 405)
			return
		}
		var req struct {
			Command string `json:"command"`
		}
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, err.Error(), 400)
			return
		}
		if req.Command == "" {
			http.Error(w, "command is required", 400)
			return
		}
		resp, err := sendCPCommand(getCPURL(), req.Command)
		if err != nil {
			w.Header().Set("Content-Type", "application/json; charset=utf-8")
			w.WriteHeader(502)
			json.NewEncoder(w).Encode(map[string]string{"status": "error", "error": err.Error()})
			return
		}
		w.Header().Set("Content-Type", "application/json; charset=utf-8")
		json.NewEncoder(w).Encode(map[string]string{"status": "ok", "response": resp})
	})

	mux.HandleFunc("/api/cp/input", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			http.Error(w, "POST only", 405)
			return
		}
		var req struct {
			Text string `json:"text"`
		}
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, err.Error(), 400)
			return
		}
		if req.Text == "" {
			http.Error(w, "text is required", 400)
			return
		}
		encoded := base64.StdEncoding.EncodeToString([]byte(req.Text))
		command := "INPUT|web-server|" + encoded
		resp, err := sendCPCommand(getCPURL(), command)
		if err != nil {
			w.Header().Set("Content-Type", "application/json; charset=utf-8")
			w.WriteHeader(502)
			json.NewEncoder(w).Encode(map[string]string{"status": "error", "error": err.Error()})
			return
		}
		w.Header().Set("Content-Type", "application/json; charset=utf-8")
		json.NewEncoder(w).Encode(map[string]string{"status": "ok", "response": resp})
	})

	// Config endpoint for frontend to discover ghostty-web port
	ghosttyPort := getGhosttyPort()
	mux.HandleFunc("/api/config", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{"ghosttyPort": ghosttyPort})
	})
	mux.HandleFunc("/api/runtime-status", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(getRuntimeStatus(repoDir))
	})

	// Reverse proxy for ghostty-web assets: /ghostty/* → localhost:{ghosttyPort}/*
	// This avoids cross-origin issues with WASM module import.
	ghosttyOrigin, _ := url.Parse("http://localhost:" + ghosttyPort)
	ghosttyProxy := httputil.NewSingleHostReverseProxy(ghosttyOrigin)
	mux.HandleFunc("/ghostty/", func(w http.ResponseWriter, r *http.Request) {
		// Strip /ghostty prefix so /ghostty/dist/foo → /dist/foo on upstream
		r.URL.Path = strings.TrimPrefix(r.URL.Path, "/ghostty")
		r.URL.RawPath = strings.TrimPrefix(r.URL.RawPath, "/ghostty")
		r.Host = ghosttyOrigin.Host
		// Remove COEP/COOP from proxied response so same-origin page works
		ghosttyProxy.ModifyResponse = func(resp *http.Response) error {
			resp.Header.Del("Cross-Origin-Embedder-Policy")
			resp.Header.Del("Cross-Origin-Opener-Policy")
			return nil
		}
		ghosttyProxy.ServeHTTP(w, r)
	})

	mux.Handle("/", http.FileServer(http.Dir(webDir)))

	addr := ":9998"
	log.Printf("Starting server on http://localhost%s", addr)
	if err := http.ListenAndServe(addr, cors(mux)); err != nil {
		log.Fatal(err)
	}
}

func handleExport(w http.ResponseWriter, r *http.Request, repoDir, format string) {
	if r.Method != "POST" {
		http.Error(w, "POST only", 405)
		return
	}

	var req struct {
		ResultPath string `json:"resultPath"`
		Output     string `json:"output"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, err.Error(), 400)
		return
	}

	snap := appState.snapshot()
	resultPath := req.ResultPath
	if resultPath == "" {
		resultPath = snap.ResultPath
	}
	if resultPath == "" {
		http.Error(w, "result path is required", 400)
		return
	}

	resultPath = resolvePath(repoDir, resultPath)
	resultPath, err := filepath.Abs(resultPath)
	if err != nil {
		http.Error(w, fmt.Sprintf("invalid result path: %v", err), 400)
		return
	}
	if !isPathAllowed(repoDir, resultPath) {
		http.Error(w, "result path not allowed", 403)
		return
	}
	if _, err := os.Stat(resultPath); err != nil {
		http.Error(w, fmt.Sprintf("result file not found: %v", err), 404)
		return
	}

	outputPath := req.Output
	if outputPath == "" {
		outputPath = defaultExportPath(resultPath, format)
	}
	outputPath, err = filepath.Abs(outputPath)
	if err != nil {
		http.Error(w, fmt.Sprintf("invalid output path: %v", err), 400)
		return
	}

	cliPath, err := findMainCLIFunc(repoDir)
	if err != nil {
		http.Error(w, err.Error(), 500)
		return
	}

	args := []string{"export", format, resultPath, "-o", outputPath}
	appState.update(func(job *JobState) {
		job.Running = true
		job.Action = "export_" + format
		job.ResultPath = resultPath
		job.LastError = ""
		job.LastStdout = ""
		job.LastStderr = ""
		job.LastExitCode = 0
	})

	stdout, stderr, exitCode, runErr := runCLIFunc(repoDir, cliPath, args)
	appState.update(func(job *JobState) {
		job.Running = false
		job.Action = "idle"
		job.LastStdout = stdout
		job.LastStderr = stderr
		job.LastExitCode = exitCode
		if runErr != nil {
			job.LastError = runErr.Error()
		}
		if exitCode == 0 {
			if format == "pdf" {
				job.PdfPath = outputPath
			} else if format == "excel" {
				job.ExcelPath = outputPath
			}
		}
	})

	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	json.NewEncoder(w).Encode(map[string]interface{}{
		"exitCode":   exitCode,
		"stdout":     stdout,
		"stderr":     stderr,
		"resultPath": resultPath,
		"outputPath": outputPath,
		"error":      errorString(runErr),
	})
}

func handleAnalyze(w http.ResponseWriter, r *http.Request, repoDir string) {
	if r.Method != "POST" {
		http.Error(w, "POST only", 405)
		return
	}
	var req struct {
		Folder      string   `json:"folder"`
		MasterFiles []string `json:"masterFiles"`
		Output      string   `json:"output"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, err.Error(), 400)
		return
	}
	if req.Folder == "" {
		http.Error(w, "folder is required", 400)
		return
	}
	cliPath, err := findMainCLIFunc(repoDir)
	if err != nil {
		http.Error(w, err.Error(), 500)
		return
	}

	folderPath, err := filepath.Abs(req.Folder)
	if err != nil {
		http.Error(w, fmt.Sprintf("invalid folder path: %v", err), 400)
		return
	}
	folderInfo, err := os.Stat(folderPath)
	if err != nil || !folderInfo.IsDir() {
		http.Error(w, "folder does not exist", 400)
		return
	}

	masterCleanup, masterPath, mergeErr := prepareMasterFileFunc(repoDir, req.MasterFiles)
	if mergeErr != nil {
		http.Error(w, fmt.Sprintf("master file error: %v", mergeErr), 500)
		return
	}
	if masterCleanup != nil {
		defer masterCleanup()
	}
	resultPath := req.Output
	if resultPath == "" {
		resultPath = filepath.Join(folderPath, "result.json")
	}
	resultPath, err = filepath.Abs(resultPath)
	if err != nil {
		http.Error(w, fmt.Sprintf("invalid output path: %v", err), 400)
		return
	}

	args := []string{"analyze", folderPath, "-o", resultPath}
	if masterPath != "" {
		args = append(args, "-m", masterPath)
	}

	appState.update(func(job *JobState) {
		job.Running = true
		job.Action = "analyze"
		job.Folder = folderPath
		job.MasterPath = masterPath
		job.ResultPath = resultPath
		job.PdfPath = ""
		job.ExcelPath = ""
		job.LastError = ""
		job.LastStdout = ""
		job.LastStderr = ""
		job.LastExitCode = 0
	})

	stdout, stderr, exitCode, runErr := runCLIFunc(repoDir, cliPath, args)
	appState.update(func(job *JobState) {
		job.Running = false
		job.Action = "idle"
		job.LastStdout = stdout
		job.LastStderr = stderr
		job.LastExitCode = exitCode
		if runErr != nil {
			job.LastError = runErr.Error()
		}
		if exitCode != 0 {
			job.PdfPath = ""
			job.ExcelPath = ""
		}
	})

	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	json.NewEncoder(w).Encode(map[string]interface{}{
		"exitCode":   exitCode,
		"stdout":     stdout,
		"stderr":     stderr,
		"resultPath": resultPath,
		"error":      errorString(runErr),
	})
}

func updateCSVRow(csvPath string, rowIndex int, row MasterRow) error {
	f, err := os.Open(csvPath)
	if err != nil {
		return err
	}
	r := csv.NewReader(f)
	allRows, err := r.ReadAll()
	f.Close()
	if err != nil {
		return err
	}
	// rowIndex is 0-based data row (header is row 0 in file)
	fileRow := rowIndex + 1
	if fileRow >= len(allRows) {
		return fmt.Errorf("row index %d out of range (max %d)", rowIndex, len(allRows)-2)
	}
	// Map columns by header
	header := allRows[0]
	if len(header) > 0 {
		header[0] = strings.TrimPrefix(header[0], "\ufeff")
	}
	colIdx := make(map[string]int)
	for i, h := range header {
		colIdx[h] = i
	}
	set := func(key, val string) {
		if idx, ok := colIdx[key]; ok && idx < len(allRows[fileRow]) {
			allRows[fileRow][idx] = val
		}
	}
	set("費目", row.Division)
	set("写真区分", row.PhotoType)
	set("工種", row.WorkType)
	set("種別", row.Variety)
	set("細別", row.Subphase)
	set("備考", row.Remarks)
	set("検索パターン", row.SearchPatterns)

	// Write back with BOM
	out, err := os.Create(csvPath)
	if err != nil {
		return err
	}
	defer out.Close()
	out.Write([]byte("\ufeff"))
	w := csv.NewWriter(out)
	w.WriteAll(allRows)
	w.Flush()
	return w.Error()
}

func renameMasterFile(repoDir, oldName, newName string) error {
	oldName = strings.TrimSpace(oldName)
	newName = strings.TrimSpace(newName)
	if oldName == "" || newName == "" {
		return errors.New("oldName and newName are required")
	}
	if strings.ContainsAny(oldName, `\/`) || strings.ContainsAny(newName, `\/`) {
		return errors.New("master name must not contain path separators")
	}
	if strings.HasSuffix(oldName, ".csv") || strings.HasSuffix(newName, ".csv") {
		return errors.New("master name must not include .csv")
	}
	if oldName == newName {
		return nil
	}

	masterDir := filepath.Join(repoDir, "master", "by_work_type")
	oldPath := filepath.Join(masterDir, oldName+".csv")
	newPath := filepath.Join(masterDir, newName+".csv")

	if _, err := os.Stat(oldPath); err != nil {
		return fmt.Errorf("source master not found: %s (%v)", oldPath, err)
	}
	if _, err := os.Stat(newPath); err == nil {
		return fmt.Errorf("target master already exists: %s", newPath)
	} else if !errors.Is(err, os.ErrNotExist) {
		return fmt.Errorf("failed to check target master: %v", err)
	}

	f, err := os.Open(oldPath)
	if err != nil {
		return fmt.Errorf("failed to open source master: %v", err)
	}

	reader := csv.NewReader(f)
	records, err := reader.ReadAll()
	if err != nil {
		f.Close()
		return fmt.Errorf("failed to read source master: %v", err)
	}
	if err := f.Close(); err != nil {
		return fmt.Errorf("failed to close source master: %v", err)
	}
	if len(records) == 0 {
		return fmt.Errorf("source master is empty: %s", oldPath)
	}

	workTypeIdx := -1
	for i, h := range records[0] {
		if strings.TrimPrefix(h, "\ufeff") == "工種" {
			workTypeIdx = i
			break
		}
	}
	if workTypeIdx >= 0 {
		for i := 1; i < len(records); i++ {
			if workTypeIdx < len(records[i]) {
				records[i][workTypeIdx] = newName
			}
		}
	}

	tmpPath := filepath.Join(masterDir, newName+".csv.tmp")
	tmpFile, err := os.Create(tmpPath)
	if err != nil {
		return fmt.Errorf("failed to create temp master: %v", err)
	}
	writer := csv.NewWriter(tmpFile)
	if err := writer.WriteAll(records); err != nil {
		tmpFile.Close()
		os.Remove(tmpPath)
		return fmt.Errorf("failed to write temp master: %v", err)
	}
	writer.Flush()
	if err := writer.Error(); err != nil {
		tmpFile.Close()
		os.Remove(tmpPath)
		return fmt.Errorf("failed to flush temp master: %v", err)
	}
	if err := tmpFile.Close(); err != nil {
		os.Remove(tmpPath)
		return fmt.Errorf("failed to close temp master: %v", err)
	}

	if err := os.Rename(tmpPath, newPath); err != nil {
		os.Remove(tmpPath)
		return fmt.Errorf("failed to move temp master into place: %v", err)
	}
	if err := os.Remove(oldPath); err != nil {
		return fmt.Errorf("renamed to %s but failed to remove old master %s: %v", newPath, oldPath, err)
	}
	return nil
}

func findMainCLI(repoDir string) (string, error) {
	candidates := []string{
		filepath.Join(repoDir, "photo-ai-go", "photo-ai.exe"),
		filepath.Join(repoDir, "photo-ai.exe"),
	}
	for _, candidate := range candidates {
		if info, err := os.Stat(candidate); err == nil && !info.IsDir() {
			return candidate, nil
		}
	}
	return "", errors.New("photo-ai.exe not found")
}

func getRuntimeStatus(repoDir string) RuntimeStatus {
	status := RuntimeStatus{
		WebReady:          true,
		AgentTerminalMode: "required_for_analysis",
		AgentOptional:     false,
	}
	if cliPath, err := findMainCLIFunc(repoDir); err == nil {
		status.MainCLIPath = cliPath
		status.MainCLIAvailable = true
		engineBinaries := resolveEngineBinaries(repoDir, cliPath)
		if p := engineBinaries["PHOTO_TAG_ENGINE_EXE"]; p != "" {
			status.TagEnginePath = p
			status.TagEngineAvailable = true
		}
		if p := engineBinaries["PHOTO_ANALYSIS_ENGINE_EXE"]; p != "" {
			status.AnalysisEnginePath = p
			status.AnalysisEnginePresent = true
		}
		if p := engineBinaries["PHOTO_PDF_ENGINE_EXE"]; p != "" {
			status.PDFEnginePath = p
			status.PDFEngineAvailable = true
		}
		if p := engineBinaries["PHOTO_EXCEL_ENGINE_EXE"]; p != "" {
			status.ExcelEnginePath = p
			status.ExcelEnginePresent = true
		}
	}
	return status
}

func resolveEngineBinaries(repoDir, cliPath string) map[string]string {
	engineNames := map[string]string{
		"PHOTO_TAG_ENGINE_EXE":      "photo-tag-engine.exe",
		"PHOTO_ANALYSIS_ENGINE_EXE": "photo-analysis-engine.exe",
		"PHOTO_PDF_ENGINE_EXE":      "photo-pdf-engine.exe",
		"PHOTO_EXCEL_ENGINE_EXE":    "photo-excel-engine.exe",
	}

	searchDirs := []string{
		filepath.Dir(cliPath),
		filepath.Join(repoDir, "target", "release"),
		filepath.Join(repoDir, "target", "debug"),
		`F:\rust-targets\release`,
		`F:\rust-targets\debug`,
	}

	resolved := make(map[string]string, len(engineNames))
	for envVar, exeName := range engineNames {
		if current := os.Getenv(envVar); current != "" {
			if info, err := os.Stat(current); err == nil && !info.IsDir() {
				resolved[envVar] = current
				continue
			}
		}
		for _, dir := range searchDirs {
			if dir == "" {
				continue
			}
			candidate := filepath.Join(dir, exeName)
			if info, err := os.Stat(candidate); err == nil && !info.IsDir() {
				resolved[envVar] = candidate
				break
			}
		}
	}
	return resolved
}

func runCLI(repoDir, cliPath string, args []string) (stdout string, stderr string, exitCode int, err error) {
	cmd := exec.Command(cliPath, args...)
	cmd.Dir = repoDir

	engineBinaries := resolveEngineBinaries(repoDir, cliPath)
	env := os.Environ()
	pathEntries := []string{filepath.Dir(cliPath)}
	for envVar, exePath := range engineBinaries {
		env = append(env, envVar+"="+exePath)
		pathEntries = append(pathEntries, filepath.Dir(exePath))
	}
	if len(pathEntries) > 0 {
		env = append(env, "PATH="+strings.Join(append(pathEntries, os.Getenv("PATH")), string(os.PathListSeparator)))
	}
	cmd.Env = env

	var outBuf, errBuf strings.Builder
	cmd.Stdout = &outBuf
	cmd.Stderr = &errBuf

	runErr := cmd.Run()
	if runErr != nil {
		if exitErr, ok := runErr.(*exec.ExitError); ok {
			return outBuf.String(), errBuf.String(), exitErr.ExitCode(), runErr
		}
		return outBuf.String(), errBuf.String(), -1, runErr
	}
	return outBuf.String(), errBuf.String(), 0, nil
}

func errorString(err error) string {
	if err == nil {
		return ""
	}
	return err.Error()
}

func defaultExportPath(resultPath, format string) string {
	dir := filepath.Dir(resultPath)
	switch format {
	case "pdf":
		return filepath.Join(dir, "工事写真帳.pdf")
	case "excel":
		return filepath.Join(dir, "工事写真帳.xlsx")
	default:
		return filepath.Join(dir, "output")
	}
}

func isWithin(basePath, targetPath string) bool {
	if basePath == "" {
		return false
	}
	baseAbs, err := filepath.Abs(basePath)
	if err != nil {
		return false
	}
	targetAbs, err := filepath.Abs(targetPath)
	if err != nil {
		return false
	}
	if baseAbs == targetAbs {
		return true
	}
	rel, err := filepath.Rel(baseAbs, targetAbs)
	if err != nil {
		return false
	}
	return rel != ".." && !strings.HasPrefix(rel, ".."+string(filepath.Separator))
}

func isPathAllowed(repoDir, targetPath string) bool {
	snap := appState.snapshot()
	candidates := []string{
		repoDir,
		snap.Folder,
		snap.ResultPath,
		snap.PdfPath,
		snap.ExcelPath,
	}
	for _, candidate := range candidates {
		if candidate == "" {
			continue
		}
		if strings.EqualFold(filepath.Ext(candidate), "") {
			if isWithin(candidate, targetPath) {
				return true
			}
			continue
		}
		if isWithin(candidate, targetPath) || isWithin(filepath.Dir(candidate), targetPath) {
			return true
		}
	}
	return false
}

// resolvePath resolves a path relative to repoDir if it is not absolute.
func resolvePath(repoDir, p string) string {
	if filepath.IsAbs(p) {
		return p
	}
	return filepath.Join(repoDir, p)
}

// prepareMasterFile handles master file resolution.
// If multiple masterFiles are given, it merges the CSVs into a temp file.
// Returns a cleanup function (may be nil), the master path, and any error.
func prepareMasterFile(repoDir string, masterFiles []string) (cleanup func(), masterPath string, err error) {
	if len(masterFiles) == 0 {
		return nil, "", nil
	}

	// Resolve each master file to its full path
	var paths []string
	for _, name := range masterFiles {
		// If it looks like a bare name (no path separator, no .csv), treat as by_work_type name
		if !strings.Contains(name, string(filepath.Separator)) && !strings.Contains(name, "/") && !strings.HasSuffix(name, ".csv") {
			name = filepath.Join("master", "by_work_type", name+".csv")
		}
		resolved := resolvePath(repoDir, name)
		if _, statErr := os.Stat(resolved); statErr != nil {
			return nil, "", fmt.Errorf("master file not found: %s (%v)", resolved, statErr)
		}
		paths = append(paths, resolved)
	}

	if len(paths) == 1 {
		return nil, paths[0], nil
	}

	// Merge multiple CSVs: keep header from first file, append data rows from the rest
	tmpFile, err := os.CreateTemp("", "merged-master-*.csv")
	if err != nil {
		return nil, "", fmt.Errorf("failed to create temp file: %v", err)
	}
	tmpPath := tmpFile.Name()

	writer := bufio.NewWriter(tmpFile)
	headerWritten := false

	for _, p := range paths {
		f, err := os.Open(p)
		if err != nil {
			tmpFile.Close()
			os.Remove(tmpPath)
			return nil, "", fmt.Errorf("failed to open %s: %v", p, err)
		}
		scanner := bufio.NewScanner(f)
		lineNum := 0
		for scanner.Scan() {
			line := scanner.Text()
			lineNum++
			if lineNum == 1 {
				if !headerWritten {
					writer.WriteString(line + "\n")
					headerWritten = true
				}
				continue
			}
			// Skip empty lines
			if strings.TrimSpace(line) == "" {
				continue
			}
			writer.WriteString(line + "\n")
		}
		f.Close()
	}

	writer.Flush()
	tmpFile.Close()

	cleanupFn := func() {
		os.Remove(tmpPath)
	}
	return cleanupFn, tmpPath, nil
}

// getCPURL returns the CP WebSocket URL from environment or default.
func getCPURL() string {
	if url := os.Getenv("GHOSTTY_CP_URL"); url != "" {
		return url
	}
	return "ws://localhost:" + getGhosttyPort() + "/cp"
}

// sendCPCommand opens a WebSocket to the CP URL, sends the command, reads the response, and closes.
func sendCPCommand(cpURL, command string) (string, error) {
	ws, err := websocket.Dial(cpURL, "", "http://localhost")
	if err != nil {
		return "", fmt.Errorf("failed to connect to CP at %s: %w", cpURL, err)
	}
	defer ws.Close()

	if err := websocket.Message.Send(ws, command); err != nil {
		return "", fmt.Errorf("failed to send CP command: %w", err)
	}

	var response string
	if err := websocket.Message.Receive(ws, &response); err != nil {
		return "", fmt.Errorf("failed to read CP response: %w", err)
	}

	return response, nil
}

func handleTerminal(ws *websocket.Conn) {
	defer ws.Close()
	log.Printf("WebSocket client connected: %s", ws.Request().RemoteAddr)

	for {
		var command string
		if err := websocket.Message.Receive(ws, &command); err != nil {
			if err == io.EOF {
				log.Printf("WebSocket client disconnected")
			} else {
				log.Printf("WebSocket read error: %v", err)
			}
			return
		}

		log.Printf("Executing command: %s", command)

		cmd := exec.Command("cmd", "/c", command)
		env := os.Environ()
		env = append(env, "NO_COLOR=1", "TERM=dumb")
		cmd.Env = env

		stdout, err := cmd.StdoutPipe()
		if err != nil {
			websocket.Message.Send(ws, fmt.Sprintf("\r\n[error] stdout pipe: %v\r\n", err))
			continue
		}
		stderr, err := cmd.StderrPipe()
		if err != nil {
			websocket.Message.Send(ws, fmt.Sprintf("\r\n[error] stderr pipe: %v\r\n", err))
			continue
		}

		if err := cmd.Start(); err != nil {
			websocket.Message.Send(ws, fmt.Sprintf("\r\n[error] start: %v\r\n", err))
			continue
		}

		var wg sync.WaitGroup
		wg.Add(2)

		ansiRe := regexp.MustCompile(`\x1b\[[0-9;]*[a-zA-Z]|\x1b\].*?\x07|\x1b\[.*?[mGKHJsu]`)

		streamPipe := func(pipe io.ReadCloser) {
			defer wg.Done()
			buf := make([]byte, 4096)
			for {
				n, err := pipe.Read(buf)
				if n > 0 {
					clean := ansiRe.ReplaceAllString(string(buf[:n]), "")
					if len(clean) > 0 {
						websocket.Message.Send(ws, clean)
					}
				}
				if err != nil {
					break
				}
			}
		}

		go streamPipe(stdout)
		go streamPipe(stderr)

		wg.Wait()

		exitCode := 0
		if err := cmd.Wait(); err != nil {
			if exitErr, ok := err.(*exec.ExitError); ok {
				exitCode = exitErr.ExitCode()
			} else {
				websocket.Message.Send(ws, fmt.Sprintf("\r\n[error] wait: %v\r\n", err))
				continue
			}
		}

		websocket.Message.Send(ws, fmt.Sprintf("\r\n[exit code: %d]\r\n", exitCode))
	}
}
