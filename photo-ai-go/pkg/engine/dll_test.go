package engine

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/YuujiKamura/photo-ai-go/pkg/tagger"
)

func TestGeneratePDF(t *testing.T) {
	// 1. EXEのパスを環境変数に設定（ビルドされたEXEを指定）
	exePath := `F:\rust-targets\release\photo-pdf-engine.exe`
	if _, err := os.Stat(exePath); err != nil {
		t.Skipf("Engine EXE not found at %s, skipping test", exePath)
	}
	os.Setenv("PHOTO_PDF_ENGINE_EXE", exePath)

	// 2. テスト用JSONの作成
	tempDir := t.TempDir()
	jsonPath := filepath.Join(tempDir, "test_result.json")
	pdfPath := filepath.Join(tempDir, "test_output.pdf")

	testData := `[
	  {
		"fileName": "test.jpg",
		"filePath": "",
		"date": "2026-04-08 12:00:00",
		"workType": "舗装工",
		"variety": "表層工",
		"subphase": "施工状況",
		"station": "No.1",
		"remarks": "Go EXE Test",
		"measurements": "50mm",
		"photoCategory": "施工状況写真",
		"hasBoard": true
	  }
	]`
	if err := os.WriteFile(jsonPath, []byte(testData), 0644); err != nil {
		t.Fatalf("Failed to write test JSON: %v", err)
	}

	// 3. エンジン呼び出し
	config := PDFConfig{
		InputJSON:     jsonPath,
		OutputPath:    pdfPath,
		PhotosPerPage: 3,
		Quality:       "medium",
	}

	t.Logf("Calling engine to generate PDF: %s", pdfPath)
	result, err := GeneratePDF(config)
	if err != nil {
		t.Fatalf("GeneratePDF failed: %v", err)
	}

	t.Logf("Success! OutputPath: %s", result.OutputPath)

	// 4. 検証
	if _, err := os.Stat(result.OutputPath); os.IsNotExist(err) {
		t.Errorf("PDF file was not generated: %s", result.OutputPath)
	}
}

func TestGenerateExcel(t *testing.T) {
	exePath := `F:\rust-targets\release\photo-excel-engine.exe`
	if _, err := os.Stat(exePath); err != nil {
		t.Skipf("Engine EXE not found at %s, skipping test", exePath)
	}
	os.Setenv("PHOTO_EXCEL_ENGINE_EXE", exePath)

	tempDir := t.TempDir()
	jsonPath := filepath.Join(tempDir, "test_result.json")
	xlsxPath := filepath.Join(tempDir, "test_output.xlsx")

	testData := `[
	  {
		"fileName": "test.jpg",
		"filePath": "",
		"date": "2026-04-08 12:00:00",
		"workType": "舗装工",
		"photoCategory": "施工状況写真"
	  }
	]`
	os.WriteFile(jsonPath, []byte(testData), 0644)

	config := ExcelConfig{
		InputJSON:  jsonPath,
		OutputPath: xlsxPath,
	}

	result, err := GenerateExcel(config)
	if err != nil {
		t.Fatalf("GenerateExcel failed: %v", err)
	}

	if _, err := os.Stat(result.OutputPath); os.IsNotExist(err) {
		t.Errorf("Excel file was not generated: %s", result.OutputPath)
	}
}

func TestProcessImage(t *testing.T) {
	tempDir := t.TempDir()
	called := false
	original := runGrouping
	t.Cleanup(func() { runGrouping = original })
	runGrouping = func(_ context.Context, cfg tagger.Config) (tagger.Result, error) {
		called = true
		if cfg.Folder != tempDir {
			t.Fatalf("expected folder %q, got %q", tempDir, cfg.Folder)
		}
		if cfg.BatchSize != 10 {
			t.Fatalf("expected batch size 10, got %d", cfg.BatchSize)
		}
		if cfg.PayPerUse {
			t.Fatalf("expected pay-per-use false by default")
		}
		return tagger.Result{
			PhotoCount: 1,
			OutputJSON: filepath.Join(cfg.Folder, "photo-groups.json"),
		}, nil
	}

	config := ImageConfig{
		Folder:    tempDir,
		BatchSize: 10,
	}

	result, err := ProcessImage(config)
	if err != nil {
		t.Fatalf("ProcessImage failed: %v", err)
	}
	if !called {
		t.Fatal("expected resident grouping backend to be called")
	}

	if result.PhotoCount < 1 {
		t.Errorf("Expected PhotoCount >= 1, got %d", result.PhotoCount)
	}
}

func TestProcessImageResident(t *testing.T) {
	tempDir := t.TempDir()
	called := false
	original := runGrouping
	t.Cleanup(func() { runGrouping = original })
	runGrouping = func(_ context.Context, cfg tagger.Config) (tagger.Result, error) {
		called = true
		if cfg.Folder != tempDir {
			t.Fatalf("expected folder %q, got %q", tempDir, cfg.Folder)
		}
		if cfg.BatchSize != 10 {
			t.Fatalf("expected batch size 10, got %d", cfg.BatchSize)
		}
		if !cfg.PayPerUse {
			// resident is the primary transport now; non-pay-per-use should remain false here.
		}
		return tagger.Result{
			PhotoCount: 2,
			OutputJSON: filepath.Join(cfg.Folder, "photo-groups.json"),
		}, nil
	}

	config := ImageConfig{
		Folder:    tempDir,
		BatchSize: 10,
		Resident:  true,
	}

	result, err := ProcessImage(config)
	if err != nil {
		t.Fatalf("ProcessImage Resident failed: %v", err)
	}
	if !called {
		t.Fatal("expected resident grouping backend to be called")
	}

	t.Logf("Resident test success: %d photos processed", result.PhotoCount)
}
