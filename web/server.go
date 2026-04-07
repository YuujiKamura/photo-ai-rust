package main

import (
	"bufio"
	"encoding/base64"
	"encoding/csv"
	"encoding/json"
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

	// Start demo server
	cmd := exec.Command("node", "bin/demo.js")
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

		// Build cargo run arguments
		args := []string{"run", "--release", "--", "analyze", req.Folder}

		// Handle master files: merge multiple CSVs into a temp file if needed
		masterCleanup, masterPath, mergeErr := prepareMasterFile(repoDir, req.MasterFiles)
		if mergeErr != nil {
			http.Error(w, fmt.Sprintf("master file error: %v", mergeErr), 500)
			return
		}
		if masterCleanup != nil {
			defer masterCleanup()
		}
		if masterPath != "" {
			args = append(args, "--master", masterPath)
		}

		if req.Output != "" {
			args = append(args, "--output", req.Output)
		}

		// If ?via=cp is set, send the cargo command through CP instead of exec
		if r.URL.Query().Get("via") == "cp" {
			fullCmd := "cargo " + strings.Join(args, " ") + "\n"
			encoded := base64.StdEncoding.EncodeToString([]byte(fullCmd))
			cpCommand := "INPUT|web-server|" + encoded
			resp, err := sendCPCommand(getCPURL(), cpCommand)
			if err != nil {
				w.Header().Set("Content-Type", "application/json; charset=utf-8")
				w.WriteHeader(502)
				json.NewEncoder(w).Encode(map[string]interface{}{
					"exitCode": -1,
					"stdout":   "",
					"stderr":   err.Error(),
					"via":      "cp",
				})
				return
			}
			w.Header().Set("Content-Type", "application/json; charset=utf-8")
			json.NewEncoder(w).Encode(map[string]interface{}{
				"exitCode": 0,
				"stdout":   resp,
				"stderr":   "",
				"via":      "cp",
			})
			return
		}

		cmd := exec.Command("cargo", args...)
		cmd.Dir = repoDir

		var stdout, stderr strings.Builder
		cmd.Stdout = &stdout
		cmd.Stderr = &stderr

		runErr := cmd.Run()
		exitCode := 0
		if runErr != nil {
			if exitErr, ok := runErr.(*exec.ExitError); ok {
				exitCode = exitErr.ExitCode()
			} else {
				http.Error(w, fmt.Sprintf("exec error: %v", runErr), 500)
				return
			}
		}

		w.Header().Set("Content-Type", "application/json; charset=utf-8")
		json.NewEncoder(w).Encode(map[string]interface{}{
			"exitCode": exitCode,
			"stdout":   stdout.String(),
			"stderr":   stderr.String(),
		})
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

		// Security: ensure the resolved path is within repoDir
		absResolved, err := filepath.Abs(resolved)
		if err != nil {
			http.Error(w, "invalid path", 400)
			return
		}
		absRepo, _ := filepath.Abs(repoDir)
		if !strings.HasPrefix(absResolved, absRepo+string(filepath.Separator)) && absResolved != absRepo {
			http.Error(w, "path traversal denied", 403)
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
		cleanup, masterPath, err := prepareMasterFile(repoDir, req.MasterFiles)
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
