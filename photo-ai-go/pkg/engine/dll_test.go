package engine

import (
	"os"
	"path/filepath"
	"testing"
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
	exePath := `F:\rust-targets\release\photo-tag-engine.exe`
	if _, err := os.Stat(exePath); err != nil {
		t.Skipf("Engine EXE not found at %s, skipping test", exePath)
	}
	os.Setenv("PHOTO_TAG_ENGINE_EXE", exePath)

	tempDir := t.TempDir()
	// Use real image if available, else fallback to dummy (which might fail)
	realImg := `C:\Users\yuuji\manual_test\image.png`
	targetImg := filepath.Join(tempDir, "image.png")
	if _, err := os.Stat(realImg); err == nil {
		// Copy real image
		data, _ := os.ReadFile(realImg)
		os.WriteFile(targetImg, data, 0644)
	} else {
		os.WriteFile(targetImg, []byte("dummy"), 0644)
	}

	config := ImageConfig{
		Folder:    tempDir,
		BatchSize: 10,
	}

	// Increase timeout for AI processing
	result, err := ProcessImage(config)
	if err != nil {
		t.Fatalf("ProcessImage failed: %v", err)
	}

	if result.PhotoCount < 1 {
		t.Errorf("Expected PhotoCount >= 1, got %d", result.PhotoCount)
	}
}

func TestProcessImageResident(t *testing.T) {
	exePath := `F:\rust-targets\release\photo-tag-engine.exe`
	if _, err := os.Stat(exePath); err != nil {
		t.Skipf("Engine EXE not found at %s, skipping test", exePath)
	}
	os.Setenv("PHOTO_TAG_ENGINE_EXE", exePath)

	// Use a known alive/dead session ID from deckpilot list for testing
	os.Setenv("DECKPILOT_SESSION", "ghostty-web-33192")

	tempDir := t.TempDir()
	realImg := `C:\Users\yuuji\manual_test\image.png`
	targetImg := filepath.Join(tempDir, "image.png")
	data, _ := os.ReadFile(realImg)
	os.WriteFile(targetImg, data, 0644)

	config := ImageConfig{
		Folder:    tempDir,
		BatchSize: 10,
		Resident:  true, // Use deckpilot mode
	}

	result, err := ProcessImage(config)
	if err != nil {
		t.Fatalf("ProcessImage Resident failed: %v", err)
	}

	t.Logf("Resident test success: %d photos processed", result.PhotoCount)
}
