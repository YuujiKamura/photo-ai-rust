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
	"net"
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

	"github.com/spf13/cobra"
	"golang.org/x/net/websocket"

	"github.com/YuujiKamura/deckpilot/daemon"
	"github.com/YuujiKamura/photo-ai-go/pkg/engine"
	embeddedmaster "github.com/YuujiKamura/photo-ai-go/internal/master"
	"github.com/YuujiKamura/photo-ai-go/internal/web"
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

var serverState AppState
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

var serveCmd = &cobra.Command{
	Use:   "serve",
	Short: "Web UIサーバーを起動",
	RunE: func(cmd *cobra.Command, args []string) error {
		port, _ := cmd.Flags().GetString("port")
		dev, _ := cmd.Flags().GetBool("dev")
		return runServer(port, dev)
	},
}

func init() {
	serveCmd.Flags().StringP("port", "p", "9998", "サーバーのポート番号")
	serveCmd.Flags().Bool("dev", false, "ローカルのweb/ディレクトリのファイルを使用する")
}

func runServer(port string, dev bool) error {
	// Start deckpilot daemon in background
	go func() {
		log.Printf("Starting deckpilot daemon...")
		d := daemon.New()
		if err := d.Run(); err != nil {
			log.Printf("deckpilot daemon error: %v", err)
		}
	}()

	var webHandler http.Handler
	var webDir string
	var repoDir string

	if dev {
		exe, _ := os.Executable()
		webDir = filepath.Join(filepath.Dir(exe), "web")
		// If running via "go run" or in dev, look for web dir in workspace
		if _, err := os.Stat(filepath.Join(webDir, "index.html")); err != nil {
			cwd, _ := os.Getwd()
			candidate := filepath.Join(cwd, "web")
			if _, err := os.Stat(filepath.Join(candidate, "index.html")); err == nil {
				webDir = candidate
			} else {
				candidate = filepath.Join(filepath.Dir(cwd), "web")
				if _, err := os.Stat(filepath.Join(candidate, "index.html")); err == nil {
					webDir = candidate
				}
			}
		}
		repoDir = filepath.Dir(webDir)
		webHandler = http.FileServer(http.Dir(webDir))
		log.Printf("Dev mode: Serving files from disk: %s", webDir)
	} else {
		webHandler = http.FileServer(http.FS(web.StaticAssets))
		// For repoDir in non-dev mode, we still might need it for master files etc.
		// We'll assume the current working directory or executable location.
		exe, _ := os.Executable()
		repoDir = filepath.Dir(exe)
		if _, err := os.Stat(filepath.Join(repoDir, "master")); err != nil {
			cwd, _ := os.Getwd()
			repoDir = cwd
		}
		log.Printf("Production mode: Serving embedded static files")
	}

	log.Printf("Repo root: %s", repoDir)

	ghosttyCmd, ghosttyCleanup := startGhosttyWeb(webDir)
	defer ghosttyCleanup()
	_ = ghosttyCmd

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)
	go func() {
		<-sigCh
		log.Printf("Shutting down...")
		ghosttyCleanup()
		os.Exit(0)
	}()

	cors := func(h http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.Header().Set("Access-Control-Allow-Origin", "*")
			w.Header().Set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
			w.Header().Set("Access-Control-Allow-Headers", "Content-Type")
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
	// ... (Rest of handlers from server.go ported here) ...
	// I'll implement them below.

	setupHandlers(mux, repoDir, webDir, webHandler)

	// Try the requested port, then fall back to alternatives
	handler := cors(mux)
	ports := []string{port, "9999", "0"}
	var listener net.Listener
	var actualPort string
	for _, p := range ports {
		addr := ":" + p
		l, err := net.Listen("tcp", addr)
		if err != nil {
			log.Printf("Port %s is in use, trying next...", p)
			continue
		}
		listener = l
		actualPort = fmt.Sprintf("%d", listener.Addr().(*net.TCPAddr).Port)
		break
	}
	if listener == nil {
		return fmt.Errorf("could not find an available port")
	}

	url := fmt.Sprintf("http://localhost:%s", actualPort)
	log.Printf("Starting server on %s", url)

	// Open browser automatically
	go func() {
		time.Sleep(300 * time.Millisecond)
		exec.Command("cmd", "/c", "start", url).Start()
	}()

	return http.Serve(listener, handler)
}

func setupHandlers(mux *http.ServeMux, repoDir, webDir string, webHandler http.Handler) {
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
		json.NewEncoder(w).Encode(serverState.snapshot())
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
		_ = cleanup
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{"path": masterPath})
	})

	mux.HandleFunc("/api/browse", func(w http.ResponseWriter, r *http.Request) {
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

	ghosttyPort := getGhosttyPort()
	mux.HandleFunc("/api/config", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{"ghosttyPort": ghosttyPort})
	})
	mux.HandleFunc("/api/runtime-status", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(getRuntimeStatus(repoDir))
	})

	if webDir != "" {
		// Dev mode: proxy to local ghostty-web demo server
		ghosttyOrigin, _ := url.Parse("http://localhost:" + ghosttyPort)
		ghosttyProxy := httputil.NewSingleHostReverseProxy(ghosttyOrigin)
		mux.HandleFunc("/ghostty/", func(w http.ResponseWriter, r *http.Request) {
			r.URL.Path = strings.TrimPrefix(r.URL.Path, "/ghostty")
			r.URL.RawPath = strings.TrimPrefix(r.URL.RawPath, "/ghostty")
			r.Host = ghosttyOrigin.Host
			ghosttyProxy.ModifyResponse = func(resp *http.Response) error {
				resp.Header.Del("Cross-Origin-Embedder-Policy")
				resp.Header.Del("Cross-Origin-Opener-Policy")
				return nil
			}
			ghosttyProxy.ServeHTTP(w, r)
		})
	} else {
		// Production mode: serve ghostty dist from embedded assets
		mux.HandleFunc("/ghostty/", func(w http.ResponseWriter, r *http.Request) {
			// /ghostty/dist/ghostty-web.js -> ghostty/dist/ghostty-web.js in embed FS
			path := strings.TrimPrefix(r.URL.Path, "/")
			data, err := web.StaticAssets.ReadFile(path)
			if err != nil {
				http.NotFound(w, r)
				return
			}
			ct := mime.TypeByExtension(filepath.Ext(path))
			if ct == "" {
				ct = "application/octet-stream"
			}
			if strings.HasSuffix(path, ".wasm") {
				ct = "application/wasm"
			}
			w.Header().Set("Content-Type", ct)
			w.Write(data)
		})
	}

	mux.Handle("/", webHandler)
}

// --- Helper Functions Ported from server.go ---

func loadMasterCSVs(repoDir string) (map[string][]MasterRow, error) {
	masterDir := filepath.Join(repoDir, "master", "by_work_type")

	// Try filesystem first, fall back to embedded master
	entries, fsErr := os.ReadDir(masterDir)
	if fsErr != nil {
		return loadMasterCSVsFromEmbed()
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
		rows := parseMasterCSV(f)
		f.Close()
		result[name] = rows
	}
	if len(result) == 0 {
		return loadMasterCSVsFromEmbed()
	}
	return result, nil
}

func loadMasterCSVsFromEmbed() (map[string][]MasterRow, error) {
	entries, err := embeddedmaster.EmbeddedMaster.ReadDir("by_work_type")
	if err != nil {
		return nil, fmt.Errorf("no master files on disk or embedded: %w", err)
	}
	log.Printf("Loading %d master files from embedded binary", len(entries))
	result := make(map[string][]MasterRow)
	for _, e := range entries {
		if e.IsDir() || !strings.HasSuffix(e.Name(), ".csv") {
			continue
		}
		name := strings.TrimSuffix(e.Name(), ".csv")
		f, err := embeddedmaster.EmbeddedMaster.Open("by_work_type/" + e.Name())
		if err != nil {
			continue
		}
		rows := parseMasterCSV(f)
		f.Close()
		result[name] = rows
	}
	return result, nil
}

func parseMasterCSV(r io.Reader) []MasterRow {
	cr := csv.NewReader(r)
	header, err := cr.Read()
	if err != nil {
		return nil
	}
	if len(header) > 0 {
		header[0] = strings.TrimPrefix(header[0], "\ufeff")
	}
	colIdx := make(map[string]int)
	for i, h := range header {
		colIdx[h] = i
	}
	var rows []MasterRow
	for {
		record, err := cr.Read()
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
	return rows
}

func getGhosttyPort() string {
	if p := os.Getenv("GHOSTTY_PORT"); p != "" {
		return p
	}
	return "8888"
}

func startGhosttyWeb(webDir string) (*exec.Cmd, func()) {
	ghosttyPort := getGhosttyPort()
	demoDir := filepath.Join(webDir, "ghostty-web", "demo")
	if _, err := os.Stat(filepath.Join(demoDir, "bin", "demo.js")); err != nil {
		log.Printf("ghostty-web submodule not found at %s, skipping", demoDir)
		return nil, func() {}
	}
	nodeModules := filepath.Join(demoDir, "node_modules")
	if _, err := os.Stat(nodeModules); err != nil {
		log.Printf("Installing ghostty-web demo dependencies...")
		install := exec.Command("npm", "install")
		install.Dir = demoDir
		install.Stdout = os.Stdout
		install.Stderr = os.Stderr
		if err := install.Run(); err != nil {
			log.Printf("npm install failed: %v", err)
			return nil, func() {}
		}
	}
	nodeCmd := "node"
	bundledNode := filepath.Join(webDir, "node.exe")
	if _, err := os.Stat(bundledNode); err == nil {
		nodeCmd = bundledNode
	}
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

func handleTerminal(ws *websocket.Conn) {
	defer ws.Close()
	log.Printf("WebSocket client connected: %s", ws.Request().RemoteAddr)
	for {
		var command string
		if err := websocket.Message.Receive(ws, &command); err != nil {
			return
		}
		log.Printf("Executing command: %s", command)
		cmd := exec.Command("cmd", "/c", command)
		env := os.Environ()
		env = append(env, "NO_COLOR=1", "TERM=dumb")
		cmd.Env = env
		stdout, _ := cmd.StdoutPipe()
		stderr, _ := cmd.StderrPipe()
		if err := cmd.Start(); err != nil {
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
			}
		}
		websocket.Message.Send(ws, fmt.Sprintf("\r\n[exit code: %d]\r\n", exitCode))
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
	fileRow := rowIndex + 1
	if fileRow >= len(allRows) {
		return fmt.Errorf("row index %d out of range", rowIndex)
	}
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
	masterDir := filepath.Join(repoDir, "master", "by_work_type")
	oldPath := filepath.Join(masterDir, oldName+".csv")
	newPath := filepath.Join(masterDir, newName+".csv")
	if _, err := os.Stat(oldPath); err != nil {
		return err
	}
	f, err := os.Open(oldPath)
	if err != nil {
		return err
	}
	reader := csv.NewReader(f)
	records, _ := reader.ReadAll()
	f.Close()
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
	tmpFile, _ := os.Create(newPath + ".tmp")
	writer := csv.NewWriter(tmpFile)
	writer.WriteAll(records)
	writer.Flush()
	tmpFile.Close()
	os.Rename(newPath+".tmp", newPath)
	os.Remove(oldPath)
	return nil
}

func findMainCLI(repoDir string) (string, error) {
	exe, _ := os.Executable()
	return exe, nil // In unified binary, it's ourselves!
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
	searchDirs := []string{}
	// Try embedded engines first (extracted to temp dir)
	if extractedDir, err := engine.EnsureEngines(); err == nil {
		searchDirs = append(searchDirs, extractedDir)
	}
	searchDirs = append(searchDirs,
		filepath.Dir(cliPath),
		filepath.Join(repoDir, "target", "release"),
		filepath.Join(repoDir, "photo-ai-go"),
	)
	resolved := make(map[string]string, len(engineNames))
	for envVar, exeName := range engineNames {
		for _, dir := range searchDirs {
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
	for envVar, exePath := range engineBinaries {
		env = append(env, envVar+"="+exePath)
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

func handleExport(w http.ResponseWriter, r *http.Request, repoDir, format string) {
	var req struct {
		ResultPath string `json:"resultPath"`
		Output     string `json:"output"`
	}
	json.NewDecoder(r.Body).Decode(&req)
	snap := serverState.snapshot()
	resultPath := req.ResultPath
	if resultPath == "" {
		resultPath = snap.ResultPath
	}
	outputPath := req.Output
	if outputPath == "" {
		outputPath = defaultExportPath(resultPath, format)
	}
	cliPath, _ := findMainCLIFunc(repoDir)
	args := []string{"export", format, resultPath, "-o", outputPath}
	serverState.update(func(job *JobState) {
		job.Running = true
		job.Action = "export_" + format
	})
	stdout, stderr, exitCode, runErr := runCLIFunc(repoDir, cliPath, args)
	serverState.update(func(job *JobState) {
		job.Running = false
		job.Action = "idle"
		job.LastStdout = stdout
		job.LastStderr = stderr
		job.LastExitCode = exitCode
		if runErr != nil {
			job.LastError = runErr.Error()
		}
	})
	json.NewEncoder(w).Encode(map[string]interface{}{"exitCode": exitCode, "stdout": stdout, "stderr": stderr})
}

func handleAnalyze(w http.ResponseWriter, r *http.Request, repoDir string) {
	var req struct {
		Folder      string   `json:"folder"`
		MasterFiles []string `json:"masterFiles"`
		Output      string   `json:"output"`
	}
	json.NewDecoder(r.Body).Decode(&req)
	cliPath, _ := findMainCLIFunc(repoDir)
	masterCleanup, masterPath, _ := prepareMasterFileFunc(repoDir, req.MasterFiles)
	if masterCleanup != nil {
		defer masterCleanup()
	}
	resultPath := req.Output
	if resultPath == "" {
		resultPath = filepath.Join(req.Folder, "result.json")
	}
	args := []string{"analyze", req.Folder, "-o", resultPath}
	if masterPath != "" {
		args = append(args, "-m", masterPath)
	}
	serverState.update(func(job *JobState) {
		job.Running = true
		job.Action = "analyze"
	})
	stdout, stderr, exitCode, runErr := runCLIFunc(repoDir, cliPath, args)
	serverState.update(func(job *JobState) {
		job.Running = false
		job.Action = "idle"
		job.LastStdout = stdout
		job.LastStderr = stderr
		job.LastExitCode = exitCode
		if runErr != nil {
			job.LastError = runErr.Error()
		}
	})
	json.NewEncoder(w).Encode(map[string]interface{}{"exitCode": exitCode, "stdout": stdout, "stderr": stderr})
}

func defaultExportPath(resultPath, format string) string {
	dir := filepath.Dir(resultPath)
	if format == "pdf" {
		return filepath.Join(dir, "工事写真帳.pdf")
	}
	return filepath.Join(dir, "工事写真帳.xlsx")
}

func isPathAllowed(repoDir, targetPath string) bool {
	return true // TODO: Implement proper security check
}

func resolvePath(repoDir, p string) string {
	if filepath.IsAbs(p) {
		return p
	}
	return filepath.Join(repoDir, p)
}

func prepareMasterFile(repoDir string, masterFiles []string) (cleanup func(), masterPath string, err error) {
	if len(masterFiles) == 0 {
		return nil, "", nil
	}
	if len(masterFiles) == 1 {
		name := masterFiles[0]
		if !strings.HasSuffix(name, ".csv") {
			name = filepath.Join("master", "by_work_type", name+".csv")
		}
		return nil, resolvePath(repoDir, name), nil
	}
	tmpFile, _ := os.CreateTemp("", "merged-master-*.csv")
	tmpPath := tmpFile.Name()
	writer := bufio.NewWriter(tmpFile)
	headerWritten := false
	for _, name := range masterFiles {
		if !strings.HasSuffix(name, ".csv") {
			name = filepath.Join("master", "by_work_type", name+".csv")
		}
		p := resolvePath(repoDir, name)
		f, _ := os.Open(p)
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
			writer.WriteString(line + "\n")
		}
		f.Close()
	}
	writer.Flush()
	tmpFile.Close()
	return func() { os.Remove(tmpPath) }, tmpPath, nil
}

func getCPURL() string {
	return "ws://localhost:" + getGhosttyPort() + "/cp"
}

func sendCPCommand(cpURL, command string) (string, error) {
	ws, err := websocket.Dial(cpURL, "", "http://localhost")
	if err != nil {
		return "", err
	}
	defer ws.Close()
	websocket.Message.Send(ws, command)
	var response string
	websocket.Message.Receive(ws, &response)
	return response, nil
}

func errorString(err error) string {
	if err == nil {
		return ""
	}
	return err.Error()
}
