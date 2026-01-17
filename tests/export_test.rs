//! PDF/Excel出力の統合テスト
//!
//! ## 変更履歴
//! - 2026-01-18: 初期作成

use photo_ai_rust::analyzer::AnalysisResult;
use photo_ai_rust::cli::PdfQuality;
use photo_ai_rust::export::{pdf, excel};
use tempfile::tempdir;

fn create_test_result(index: usize) -> AnalysisResult {
    AnalysisResult {
        file_name: format!("test_{}.jpg", index),
        file_path: String::new(),
        date: "2026-01-18".to_string(),
        work_type: "舗装工事".to_string(),
        variety: "表層工".to_string(),
        detail: format!("テスト写真{}", index),
        station: format!("No.{}+0.0", index * 10),
        remarks: "備考テスト".to_string(),
        description: format!("テスト説明{}", index),
        has_board: false,
        photo_category: "施工状況".to_string(),
        measurements: "50mm".to_string(),
        detected_text: String::new(),
        reasoning: String::new(),
    }
}

#[test]
fn test_pdf_generation_without_images() {
    let dir = tempdir().expect("Failed to create temp dir");
    let output_path = dir.path().join("test_output.pdf");

    let results: Vec<AnalysisResult> = (1..=3)
        .map(|i| create_test_result(i))
        .collect();

    let result = pdf::generate_pdf(
        &results,
        &output_path,
        3, // photos_per_page
        "テスト写真帳",
        PdfQuality::Medium,
    );

    assert!(result.is_ok(), "PDF生成に失敗: {:?}", result.err());
    assert!(output_path.exists(), "PDFファイルが作成されていない");

    let metadata = std::fs::metadata(&output_path).expect("ファイルメタデータ取得失敗");
    assert!(metadata.len() > 0, "PDFファイルが空");

    println!("PDF size: {} bytes", metadata.len());
}

#[test]
fn test_pdf_generation_empty_results() {
    let dir = tempdir().expect("Failed to create temp dir");
    let output_path = dir.path().join("empty.pdf");

    let results: Vec<AnalysisResult> = vec![];

    let result = pdf::generate_pdf(
        &results,
        &output_path,
        3,
        "空のテスト",
        PdfQuality::Medium,
    );

    // 空の結果でも正常に処理されるべき
    assert!(result.is_ok(), "空のPDF生成に失敗: {:?}", result.err());
}

#[test]
fn test_excel_generation() {
    let dir = tempdir().expect("Failed to create temp dir");
    let output_path = dir.path().join("test_output.xlsx");

    let results: Vec<AnalysisResult> = (1..=5)
        .map(|i| create_test_result(i))
        .collect();

    let result = excel::generate_excel(&results, &output_path, "テスト写真帳");

    assert!(result.is_ok(), "Excel生成に失敗: {:?}", result.err());
    assert!(output_path.exists(), "Excelファイルが作成されていない");

    let metadata = std::fs::metadata(&output_path).expect("ファイルメタデータ取得失敗");
    assert!(metadata.len() > 0, "Excelファイルが空");

    println!("Excel size: {} bytes", metadata.len());
}

#[test]
fn test_excel_generation_empty_results() {
    let dir = tempdir().expect("Failed to create temp dir");
    let output_path = dir.path().join("empty.xlsx");

    let results: Vec<AnalysisResult> = vec![];

    let result = excel::generate_excel(&results, &output_path, "空のテスト");

    assert!(result.is_ok(), "空のExcel生成に失敗: {:?}", result.err());
}

#[test]
fn test_pdf_quality_options() {
    let dir = tempdir().expect("Failed to create temp dir");

    let results: Vec<AnalysisResult> = (1..=2)
        .map(|i| create_test_result(i))
        .collect();

    for quality in [PdfQuality::Low, PdfQuality::Medium, PdfQuality::High] {
        let output_path = dir.path().join(format!("test_{:?}.pdf", quality));

        let result = pdf::generate_pdf(
            &results,
            &output_path,
            3,
            &format!("品質テスト {:?}", quality),
            quality,
        );

        assert!(result.is_ok(), "PDF生成({:?})に失敗: {:?}", quality, result.err());
        assert!(output_path.exists(), "PDFファイル({:?})が作成されていない", quality);
    }
}
