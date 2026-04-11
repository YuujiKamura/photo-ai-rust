package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func resetTestState(t *testing.T) {
	t.Helper()
	appState = AppState{}
	findMainCLIFunc = findMainCLI
	runCLIFunc = runCLI
	prepareMasterFileFunc = prepareMasterFile
	t.Cleanup(func() {
		appState = AppState{}
		findMainCLIFunc = findMainCLI
		runCLIFunc = runCLI
		prepareMasterFileFunc = prepareMasterFile
	})
}

func mustWriteFile(t *testing.T, path string, data string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatalf("mkdir %s: %v", path, err)
	}
	if err := os.WriteFile(path, []byte(data), 0o644); err != nil {
		t.Fatalf("write %s: %v", path, err)
	}
}

func newJSONRequest(t *testing.T, method string, body any) *http.Request {
	t.Helper()
	var buf bytes.Buffer
	if body != nil {
		if err := json.NewEncoder(&buf).Encode(body); err != nil {
			t.Fatalf("encode request: %v", err)
		}
	}
	req := httptest.NewRequest(method, "/", &buf)
	req.Header.Set("Content-Type", "application/json")
	return req
}

func decodeJSONMap(t *testing.T, rr *httptest.ResponseRecorder) map[string]any {
	t.Helper()
	var out map[string]any
	if err := json.Unmarshal(rr.Body.Bytes(), &out); err != nil {
		t.Fatalf("decode json: %v\nbody=%s", err, rr.Body.String())
	}
	return out
}

func TestDefaultExportPathPDF(t *testing.T) {
	got := defaultExportPath(filepath.Join(`C:\tmp`, "result.json"), "pdf")
	if !strings.HasSuffix(got, `工事写真帳.pdf`) {
		t.Fatalf("expected pdf path, got %s", got)
	}
}

func TestDefaultExportPathExcel(t *testing.T) {
	got := defaultExportPath(filepath.Join(`C:\tmp`, "result.json"), "excel")
	if !strings.HasSuffix(got, `工事写真帳.xlsx`) {
		t.Fatalf("expected excel path, got %s", got)
	}
}

func TestDefaultExportPathFallback(t *testing.T) {
	got := defaultExportPath(filepath.Join(`C:\tmp`, "result.json"), "other")
	if !strings.HasSuffix(got, `output`) {
		t.Fatalf("expected fallback path, got %s", got)
	}
}

func TestIsWithinSamePath(t *testing.T) {
	if !isWithin(`C:\tmp\repo`, `C:\tmp\repo`) {
		t.Fatal("expected same path to be within")
	}
}

func TestIsWithinChildPath(t *testing.T) {
	if !isWithin(`C:\tmp\repo`, `C:\tmp\repo\child\file.json`) {
		t.Fatal("expected child path to be within")
	}
}

func TestIsWithinOutsidePath(t *testing.T) {
	if isWithin(`C:\tmp\repo`, `C:\tmp\other\file.json`) {
		t.Fatal("expected outside path to be rejected")
	}
}

func TestResolvePathRelative(t *testing.T) {
	got := resolvePath(`C:\repo`, `foo\bar.json`)
	if got != filepath.Join(`C:\repo`, `foo\bar.json`) {
		t.Fatalf("unexpected path: %s", got)
	}
}

func TestResolvePathAbsolute(t *testing.T) {
	got := resolvePath(`C:\repo`, `C:\data\result.json`)
	if got != `C:\data\result.json` {
		t.Fatalf("unexpected path: %s", got)
	}
}

func TestFindMainCLIPrefersPhotoAIGo(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	goCLI := filepath.Join(repo, "photo-ai-go", "photo-ai.exe")
	rootCLI := filepath.Join(repo, "photo-ai.exe")
	mustWriteFile(t, goCLI, "go")
	mustWriteFile(t, rootCLI, "root")

	got, err := findMainCLI(repo)
	if err != nil {
		t.Fatalf("findMainCLI: %v", err)
	}
	if got != goCLI {
		t.Fatalf("expected go cli first, got %s", got)
	}
}

func TestFindMainCLIFallsBackToRoot(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	rootCLI := filepath.Join(repo, "photo-ai.exe")
	mustWriteFile(t, rootCLI, "root")

	got, err := findMainCLI(repo)
	if err != nil {
		t.Fatalf("findMainCLI: %v", err)
	}
	if got != rootCLI {
		t.Fatalf("expected root cli, got %s", got)
	}
}

func TestResolveEngineBinariesUsesEnvOverride(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	cliPath := filepath.Join(repo, "photo-ai-go", "photo-ai.exe")
	mustWriteFile(t, cliPath, "cli")
	override := filepath.Join(t.TempDir(), "photo-analysis-engine.exe")
	mustWriteFile(t, override, "analysis")
	t.Setenv("PHOTO_ANALYSIS_ENGINE_EXE", override)

	got := resolveEngineBinaries(repo, cliPath)
	if got["PHOTO_ANALYSIS_ENGINE_EXE"] != override {
		t.Fatalf("expected env override, got %#v", got)
	}
}

func TestGetRuntimeStatusWithoutCLI(t *testing.T) {
	resetTestState(t)
	t.Setenv("PHOTO_AI_MASTER_DIR", filepath.Join(t.TempDir(), "user-master"))
	status := getRuntimeStatus(t.TempDir())
	if status.MainCLIAvailable {
		t.Fatal("cli should be unavailable")
	}
	if status.AgentOptional {
		t.Fatal("agent should not be optional for analysis")
	}
	if status.AgentTerminalMode != "required_for_analysis" {
		t.Fatalf("unexpected agent mode: %q", status.AgentTerminalMode)
	}
	if status.MasterSource != "user" {
		t.Fatalf("expected user master source, got %#v", status)
	}
}

func TestGetRuntimeStatusWithCLIAndEngines(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	t.Setenv("PHOTO_AI_MASTER_DIR", filepath.Join(t.TempDir(), "user-master"))
	cliPath := filepath.Join(repo, "photo-ai-go", "photo-ai.exe")
	mustWriteFile(t, cliPath, "cli")
	mustWriteFile(t, filepath.Join(filepath.Dir(cliPath), "photo-analysis-engine.exe"), "analysis")
	mustWriteFile(t, filepath.Join(filepath.Dir(cliPath), "photo-pdf-engine.exe"), "pdf")
	mustWriteFile(t, filepath.Join(filepath.Dir(cliPath), "photo-excel-engine.exe"), "excel")

	status := getRuntimeStatus(repo)
	if !status.MainCLIAvailable || !status.AnalysisEnginePresent || !status.PDFEngineAvailable || !status.ExcelEnginePresent {
		t.Fatalf("expected all runtime components, got %#v", status)
	}
	if status.MasterVersion == "" || status.MasterSchemaVersion == 0 {
		t.Fatalf("expected master metadata, got %#v", status)
	}
}

func TestPrepareMasterFileNoFiles(t *testing.T) {
	resetTestState(t)
	cleanup, path, err := prepareMasterFile(t.TempDir(), nil)
	if err != nil || cleanup != nil || path != "" {
		t.Fatalf("unexpected result: cleanupNil=%t path=%q err=%v", cleanup == nil, path, err)
	}
}

func TestPrepareMasterFileSingleBareName(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	userRoot := filepath.Join(t.TempDir(), "user-master")
	t.Setenv("PHOTO_AI_MASTER_DIR", userRoot)
	master := filepath.Join(userRoot, "by_work_type", "舗装工.csv")
	mustWriteFile(t, master, "h1,h2\n1,2\n")
	mustWriteFile(t, filepath.Join(userRoot, "manifest.json"), "{\"schema_version\":1,\"master_version\":\"test\",\"files\":[\"by_work_type/舗装工.csv\"]}\n")

	cleanup, path, err := prepareMasterFile(repo, []string{"舗装工"})
	if err != nil {
		t.Fatalf("prepareMasterFile: %v", err)
	}
	if cleanup != nil {
		t.Fatal("did not expect cleanup for single file")
	}
	if path != master {
		t.Fatalf("expected %s, got %s", master, path)
	}
}

func TestPrepareMasterFileMergeMultiple(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	userRoot := filepath.Join(t.TempDir(), "user-master")
	t.Setenv("PHOTO_AI_MASTER_DIR", userRoot)
	first := filepath.Join(userRoot, "by_work_type", "舗装工.csv")
	second := filepath.Join(userRoot, "by_work_type", "共通.csv")
	mustWriteFile(t, first, "h1,h2\n1,2\n")
	mustWriteFile(t, second, "h1,h2\n3,4\n")
	mustWriteFile(t, filepath.Join(userRoot, "manifest.json"), "{\"schema_version\":1,\"master_version\":\"test\",\"files\":[\"by_work_type/舗装工.csv\",\"by_work_type/共通.csv\"]}\n")

	cleanup, path, err := prepareMasterFile(repo, []string{"舗装工", "共通"})
	if err != nil {
		t.Fatalf("prepareMasterFile: %v", err)
	}
	if cleanup == nil {
		t.Fatal("expected cleanup for merged file")
	}
	defer cleanup()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read merged file: %v", err)
	}
	text := string(data)
	if !strings.Contains(text, "1,2") || !strings.Contains(text, "3,4") {
		t.Fatalf("merged file missing rows: %s", text)
	}
}

func TestPrepareMasterFileMissing(t *testing.T) {
	resetTestState(t)
	t.Setenv("PHOTO_AI_MASTER_DIR", filepath.Join(t.TempDir(), "user-master"))
	_, _, err := prepareMasterFile(t.TempDir(), []string{"missing"})
	if err == nil {
		t.Fatal("expected missing master error")
	}
}

func TestRenameMasterFileRenamesCSVAndUpdatesWorkTypeColumn(t *testing.T) {
	resetTestState(t)
	root := t.TempDir()
	oldPath := filepath.Join(root, "by_work_type", "仮設工.csv")
	mustWriteFile(t, oldPath, "費目,写真区分,工種,種別,細別,備考,検索パターン\n共通,施工状況写真,仮設工,仮設,準備,着工前,着工前\n")

	if err := renameMasterFile(root, "仮設工", "共通仮設"); err != nil {
		t.Fatalf("renameMasterFile: %v", err)
	}

	if _, err := os.Stat(oldPath); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("expected old master removed, got err=%v", err)
	}

	newPath := filepath.Join(repo, "master", "by_work_type", "共通仮設.csv")
	data, err := os.ReadFile(newPath)
	if err != nil {
		t.Fatalf("read renamed master: %v", err)
	}
	text := string(data)
	if !strings.Contains(text, "共通仮設") {
		t.Fatalf("expected renamed work type in csv, got %s", text)
	}
	if strings.Contains(text, "仮設工,仮設") {
		t.Fatalf("expected old work type value to be updated, got %s", text)
	}
}

func TestRenameMasterFileRejectsExistingTarget(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	mustWriteFile(t, filepath.Join(repo, "master", "by_work_type", "仮設工.csv"), "費目,写真区分,工種\n共通,施工状況写真,仮設工\n")
	mustWriteFile(t, filepath.Join(repo, "master", "by_work_type", "共通仮設.csv"), "費目,写真区分,工種\n共通,施工状況写真,共通仮設\n")

	err := renameMasterFile(repo, "仮設工", "共通仮設")
	if err == nil {
		t.Fatal("expected rename conflict")
	}
}

func TestIsPathAllowedForRepoFile(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	target := filepath.Join(repo, "result.json")
	if !isPathAllowed(repo, target) {
		t.Fatal("expected repo file to be allowed")
	}
}

func TestIsPathAllowedForJobFolderFile(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	folder := filepath.Join(t.TempDir(), "photos")
	appState.update(func(job *JobState) { job.Folder = folder })
	target := filepath.Join(folder, "result.json")
	if !isPathAllowed(repo, target) {
		t.Fatal("expected folder file to be allowed")
	}
}

func TestIsPathAllowedForResultPathSibling(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	resultPath := filepath.Join(t.TempDir(), "case", "result.json")
	appState.update(func(job *JobState) { job.ResultPath = resultPath })
	target := filepath.Join(filepath.Dir(resultPath), "工事写真帳.pdf")
	if !isPathAllowed(repo, target) {
		t.Fatal("expected sibling under result dir to be allowed")
	}
}

func TestIsPathAllowedRejectsOutside(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	target := filepath.Join(t.TempDir(), "outside", "result.json")
	if isPathAllowed(repo, target) {
		t.Fatal("expected outside path to be rejected")
	}
}

func TestHandleAnalyzeRejectsGET(t *testing.T) {
	resetTestState(t)
	rr := httptest.NewRecorder()
	handleAnalyze(rr, httptest.NewRequest(http.MethodGet, "/api/analyze", nil), t.TempDir())
	if rr.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d", rr.Code)
	}
}

func TestHandleAnalyzeRequiresFolder(t *testing.T) {
	resetTestState(t)
	rr := httptest.NewRecorder()
	handleAnalyze(rr, newJSONRequest(t, http.MethodPost, map[string]any{}), t.TempDir())
	if rr.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", rr.Code)
	}
}

func TestHandleAnalyzeRejectsMissingFolderDir(t *testing.T) {
	resetTestState(t)
	findMainCLIFunc = func(repoDir string) (string, error) { return "photo-ai.exe", nil }
	rr := httptest.NewRecorder()
	handleAnalyze(rr, newJSONRequest(t, http.MethodPost, map[string]any{"folder": filepath.Join(t.TempDir(), "missing")}), t.TempDir())
	if rr.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", rr.Code)
	}
}

func TestHandleAnalyzeFailsCLILookup(t *testing.T) {
	resetTestState(t)
	findMainCLIFunc = func(repoDir string) (string, error) { return "", errors.New("cli missing") }
	folder := t.TempDir()
	rr := httptest.NewRecorder()
	handleAnalyze(rr, newJSONRequest(t, http.MethodPost, map[string]any{"folder": folder}), t.TempDir())
	if rr.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", rr.Code)
	}
}

func TestHandleAnalyzeFailsMasterPreparation(t *testing.T) {
	resetTestState(t)
	findMainCLIFunc = func(repoDir string) (string, error) { return "photo-ai.exe", nil }
	prepareMasterFileFunc = func(repoDir string, masterFiles []string) (func(), string, error) {
		return nil, "", errors.New("merge failed")
	}
	folder := t.TempDir()
	rr := httptest.NewRecorder()
	handleAnalyze(rr, newJSONRequest(t, http.MethodPost, map[string]any{"folder": folder, "masterFiles": []string{"舗装工"}}), t.TempDir())
	if rr.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", rr.Code)
	}
}

func TestHandleAnalyzeSuccessUpdatesState(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	folder := t.TempDir()
	findMainCLIFunc = func(repoDir string) (string, error) { return "photo-ai.exe", nil }
	prepareMasterFileFunc = func(repoDir string, masterFiles []string) (func(), string, error) {
		return nil, filepath.Join(repo, "master.csv"), nil
	}
	runCLIFunc = func(repoDir, cliPath string, args []string) (string, string, int, error) {
		if cliPath != "photo-ai.exe" {
			t.Fatalf("unexpected cli path: %s", cliPath)
		}
		if len(args) < 4 || args[0] != "analyze" {
			t.Fatalf("unexpected args: %#v", args)
		}
		return "ok", "", 0, nil
	}

	rr := httptest.NewRecorder()
	handleAnalyze(rr, newJSONRequest(t, http.MethodPost, map[string]any{"folder": folder}), repo)
	if rr.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d body=%s", rr.Code, rr.Body.String())
	}
	snap := appState.snapshot()
	if snap.Running {
		t.Fatal("job should not be running after completion")
	}
	if snap.ResultPath == "" || snap.Folder == "" {
		t.Fatalf("expected folder/result paths, got %#v", snap)
	}
	resp := decodeJSONMap(t, rr)
	if resp["exitCode"].(float64) != 0 {
		t.Fatalf("expected exitCode 0, got %#v", resp)
	}
}

func TestHandleAnalyzeFailureSetsLastError(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	folder := t.TempDir()
	findMainCLIFunc = func(repoDir string) (string, error) { return "photo-ai.exe", nil }
	runCLIFunc = func(repoDir, cliPath string, args []string) (string, string, int, error) {
		return "", "boom", 1, errors.New("exit status 1")
	}

	rr := httptest.NewRecorder()
	handleAnalyze(rr, newJSONRequest(t, http.MethodPost, map[string]any{"folder": folder}), repo)
	if rr.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", rr.Code)
	}
	snap := appState.snapshot()
	if snap.LastError == "" || snap.LastExitCode != 1 {
		t.Fatalf("expected error state, got %#v", snap)
	}
}

func TestHandleAnalyzeOverwritesSameResultPathOnRerun(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	folder := t.TempDir()
	resultPath := filepath.Join(folder, "result.json")
	findMainCLIFunc = func(repoDir string) (string, error) { return "photo-ai.exe", nil }
	prepareMasterFileFunc = func(repoDir string, masterFiles []string) (func(), string, error) {
		if len(masterFiles) == 0 {
			return nil, "", nil
		}
		return nil, filepath.Join(repoDir, "master", "by_work_type", masterFiles[0]+".csv"), nil
	}
	runCLIFunc = func(repoDir, cliPath string, args []string) (string, string, int, error) {
		var body string
		if strings.Contains(strings.Join(args, " "), "舗装工.csv") {
			body = `[{"fileName":"IMG_001.jpg","remarks":"舗装工結果"}]`
		} else {
			body = `[{"fileName":"IMG_001.jpg","remarks":"共通結果"}]`
		}
		if err := os.WriteFile(resultPath, []byte(body), 0o644); err != nil {
			t.Fatalf("write result: %v", err)
		}
		return "ok", "", 0, nil
	}

	firstReq := newJSONRequest(t, http.MethodPost, map[string]any{"folder": folder, "masterFiles": []string{"舗装工"}})
	firstRR := httptest.NewRecorder()
	handleAnalyze(firstRR, firstReq, repo)
	if firstRR.Code != http.StatusOK {
		t.Fatalf("expected first analyze 200, got %d body=%s", firstRR.Code, firstRR.Body.String())
	}
	firstData, err := os.ReadFile(resultPath)
	if err != nil {
		t.Fatalf("read first result: %v", err)
	}
	if !strings.Contains(string(firstData), "舗装工結果") {
		t.Fatalf("expected first result content, got %s", string(firstData))
	}

	secondReq := newJSONRequest(t, http.MethodPost, map[string]any{"folder": folder, "masterFiles": []string{"共通"}})
	secondRR := httptest.NewRecorder()
	handleAnalyze(secondRR, secondReq, repo)
	if secondRR.Code != http.StatusOK {
		t.Fatalf("expected second analyze 200, got %d body=%s", secondRR.Code, secondRR.Body.String())
	}
	secondData, err := os.ReadFile(resultPath)
	if err != nil {
		t.Fatalf("read second result: %v", err)
	}
	if !strings.Contains(string(secondData), "共通結果") {
		t.Fatalf("expected second result content, got %s", string(secondData))
	}
	if strings.Contains(string(secondData), "舗装工結果") {
		t.Fatalf("expected old result to be overwritten, got %s", string(secondData))
	}
}

func TestHandleExportRejectsGET(t *testing.T) {
	resetTestState(t)
	rr := httptest.NewRecorder()
	handleExport(rr, httptest.NewRequest(http.MethodGet, "/api/export/pdf", nil), t.TempDir(), "pdf")
	if rr.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d", rr.Code)
	}
}

func TestHandleExportRequiresResultPath(t *testing.T) {
	resetTestState(t)
	rr := httptest.NewRecorder()
	handleExport(rr, newJSONRequest(t, http.MethodPost, map[string]any{}), t.TempDir(), "pdf")
	if rr.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", rr.Code)
	}
}

func TestHandleExportRejectsDisallowedPath(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	other := filepath.Join(t.TempDir(), "outside", "result.json")
	mustWriteFile(t, other, "[]")
	rr := httptest.NewRecorder()
	handleExport(rr, newJSONRequest(t, http.MethodPost, map[string]any{"resultPath": other}), repo, "pdf")
	if rr.Code != http.StatusForbidden {
		t.Fatalf("expected 403, got %d", rr.Code)
	}
}

func TestHandleExportRejectsMissingResultFile(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	folder := filepath.Join(repo, "case")
	appState.update(func(job *JobState) { job.Folder = folder })
	result := filepath.Join(folder, "result.json")
	rr := httptest.NewRecorder()
	handleExport(rr, newJSONRequest(t, http.MethodPost, map[string]any{"resultPath": result}), repo, "pdf")
	if rr.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d", rr.Code)
	}
}

func TestHandleExportSuccessSetsPDFPath(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	folder := filepath.Join(repo, "case")
	result := filepath.Join(folder, "result.json")
	mustWriteFile(t, result, "[]")
	appState.update(func(job *JobState) { job.Folder = folder })
	findMainCLIFunc = func(repoDir string) (string, error) { return "photo-ai.exe", nil }
	runCLIFunc = func(repoDir, cliPath string, args []string) (string, string, int, error) {
		return "", "", 0, nil
	}

	rr := httptest.NewRecorder()
	handleExport(rr, newJSONRequest(t, http.MethodPost, map[string]any{"resultPath": result}), repo, "pdf")
	if rr.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d body=%s", rr.Code, rr.Body.String())
	}
	snap := appState.snapshot()
	if !strings.HasSuffix(snap.PdfPath, `工事写真帳.pdf`) {
		t.Fatalf("expected pdf path, got %#v", snap)
	}
}

func TestHandleExportSuccessSetsExcelPath(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	folder := filepath.Join(repo, "case")
	result := filepath.Join(folder, "result.json")
	mustWriteFile(t, result, "[]")
	appState.update(func(job *JobState) { job.Folder = folder })
	findMainCLIFunc = func(repoDir string) (string, error) { return "photo-ai.exe", nil }
	runCLIFunc = func(repoDir, cliPath string, args []string) (string, string, int, error) {
		return "", "", 0, nil
	}

	rr := httptest.NewRecorder()
	handleExport(rr, newJSONRequest(t, http.MethodPost, map[string]any{"resultPath": result}), repo, "excel")
	if rr.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d body=%s", rr.Code, rr.Body.String())
	}
	snap := appState.snapshot()
	if !strings.HasSuffix(snap.ExcelPath, `工事写真帳.xlsx`) {
		t.Fatalf("expected excel path, got %#v", snap)
	}
}

func TestHandleExportFailureSetsLastError(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	folder := filepath.Join(repo, "case")
	result := filepath.Join(folder, "result.json")
	mustWriteFile(t, result, "[]")
	appState.update(func(job *JobState) { job.Folder = folder })
	findMainCLIFunc = func(repoDir string) (string, error) { return "photo-ai.exe", nil }
	runCLIFunc = func(repoDir, cliPath string, args []string) (string, string, int, error) {
		return "", "bad", 1, errors.New("exit status 1")
	}

	rr := httptest.NewRecorder()
	handleExport(rr, newJSONRequest(t, http.MethodPost, map[string]any{"resultPath": result}), repo, "pdf")
	if rr.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d body=%s", rr.Code, rr.Body.String())
	}
	snap := appState.snapshot()
	if snap.LastError == "" || snap.LastExitCode != 1 {
		t.Fatalf("expected export error state, got %#v", snap)
	}
}

func TestAPIFileServesAllowedImage(t *testing.T) {
	resetTestState(t)
	repo := t.TempDir()
	folder := filepath.Join(repo, "case")
	imagePath := filepath.Join(folder, "IMG_001.jpg")
	mustWriteFile(t, imagePath, "fakejpg")
	appState.update(func(job *JobState) { job.Folder = folder })

	mux := http.NewServeMux()
	mux.HandleFunc("/api/file", func(w http.ResponseWriter, r *http.Request) {
		filePath := r.URL.Query().Get("path")
		if filePath == "" {
			http.Error(w, "missing ?path= parameter", 400)
			return
		}
		resolved := resolvePath(repo, filePath)
		if !isPathAllowed(repo, resolved) {
			http.Error(w, "path traversal denied", 403)
			return
		}
		absResolved, err := filepath.Abs(resolved)
		if err != nil {
			http.Error(w, "invalid path", 400)
			return
		}
		http.ServeFile(w, r, absResolved)
	})

	req := httptest.NewRequest(http.MethodGet, "/api/file?path="+imagePath, nil)
	rr := httptest.NewRecorder()
	mux.ServeHTTP(rr, req)
	if rr.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d body=%s", rr.Code, rr.Body.String())
	}
	if body := rr.Body.String(); body != "fakejpg" {
		t.Fatalf("unexpected body: %q", body)
	}
}
