//! PDF/Excel出力の統合テスト
//!
//! ## 変更履歴
//! - 2026-01-18: 初期作成
//! - 2026-01-18: PDF/Excel整合性テスト追加

use calamine::{Reader, Xlsx, open_workbook};
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

/// PDF/Excel両方を同一データから生成できることを確認
#[test]
fn test_pdf_excel_both_generation() {
    let dir = tempdir().expect("Failed to create temp dir");
    let pdf_path = dir.path().join("consistency.pdf");
    let excel_path = dir.path().join("consistency.xlsx");

    let results: Vec<AnalysisResult> = (1..=3)
        .map(|i| create_test_result(i))
        .collect();

    // PDF生成
    let pdf_result = pdf::generate_pdf(
        &results,
        &pdf_path,
        3,
        "整合性テスト",
        PdfQuality::Medium,
    );
    assert!(pdf_result.is_ok(), "PDF生成に失敗: {:?}", pdf_result.err());

    // Excel生成
    let excel_result = excel::generate_excel(&results, &excel_path, "整合性テスト");
    assert!(excel_result.is_ok(), "Excel生成に失敗: {:?}", excel_result.err());

    // 両方のファイルが存在することを確認
    assert!(pdf_path.exists(), "PDFファイルが作成されていない");
    assert!(excel_path.exists(), "Excelファイルが作成されていない");
}

/// Excelに書き込んだデータが正しく読み戻せることを確認
#[test]
fn test_excel_data_consistency() {
    let dir = tempdir().expect("Failed to create temp dir");
    let excel_path = dir.path().join("data_check.xlsx");

    // テストデータ作成（特定の値で検証しやすくする）
    let results = vec![
        AnalysisResult {
            file_name: "IMG_001.jpg".to_string(),
            file_path: String::new(),
            date: "2026-01-15".to_string(),
            work_type: "舗装工事".to_string(),
            variety: "表層工".to_string(),
            detail: "アスファルト舗設".to_string(),
            station: "No.5+10.0".to_string(),
            remarks: "1層目".to_string(),
            description: String::new(),
            has_board: false,
            photo_category: "施工状況".to_string(),
            measurements: "t=50mm".to_string(),
            detected_text: String::new(),
            reasoning: String::new(),
        },
        AnalysisResult {
            file_name: "IMG_002.jpg".to_string(),
            file_path: String::new(),
            date: "2026-01-16".to_string(),
            work_type: "舗装工事".to_string(),
            variety: "表層工".to_string(),
            detail: "温度測定".to_string(),
            station: "No.5+10.0".to_string(),
            remarks: "到着温度".to_string(),
            description: String::new(),
            has_board: false,
            photo_category: "品質管理".to_string(),
            measurements: "158℃".to_string(),
            detected_text: String::new(),
            reasoning: String::new(),
        },
    ];

    // Excel生成
    excel::generate_excel(&results, &excel_path, "データ検証")
        .expect("Excel生成に失敗");

    // Excelを読み戻して検証
    let mut workbook: Xlsx<_> = open_workbook(&excel_path)
        .expect("Excelファイルを開けない");

    let sheet_name = workbook.sheet_names().first().cloned()
        .expect("シートがない");
    let range = workbook.worksheet_range(&sheet_name)
        .expect("シートを読み込めない");

    // ヘッダー確認（1行目）
    let headers: Vec<String> = (0..range.width())
        .map(|col| {
            range.get_value((0, col as u32))
                .map(|v| v.to_string())
                .unwrap_or_default()
        })
        .collect();

    assert!(headers.contains(&"ファイル名".to_string()), "ヘッダーにファイル名がない");
    assert!(headers.contains(&"日時".to_string()), "ヘッダーに日時がない");
    assert!(headers.contains(&"工種".to_string()), "ヘッダーに工種がない");

    // データ行数確認（ヘッダー1行 + データ2行 = 3行）
    assert_eq!(range.height(), 3, "行数が一致しない");

    // 1行目のデータ確認
    let row1_file = range.get_value((1, 0))
        .map(|v| v.to_string())
        .unwrap_or_default();
    assert_eq!(row1_file, "IMG_001.jpg", "ファイル名が一致しない");

    let row1_date = range.get_value((1, 1))
        .map(|v| v.to_string())
        .unwrap_or_default();
    assert_eq!(row1_date, "2026-01-15", "日時が一致しない");

    // 2行目のデータ確認
    let row2_file = range.get_value((2, 0))
        .map(|v| v.to_string())
        .unwrap_or_default();
    assert_eq!(row2_file, "IMG_002.jpg", "2行目ファイル名が一致しない");

    let row2_measurements = range.get_value((2, 8))
        .map(|v| v.to_string())
        .unwrap_or_default();
    assert_eq!(row2_measurements, "158℃", "測定値が一致しない");

    println!("Excel整合性検証: 全項目一致");
}

/// 日本語文字が正しく保存・読み戻しされることを確認
#[test]
fn test_excel_japanese_text() {
    let dir = tempdir().expect("Failed to create temp dir");
    let excel_path = dir.path().join("japanese_test.xlsx");

    let results = vec![
        AnalysisResult {
            file_name: "写真001.jpg".to_string(),
            file_path: String::new(),
            date: "令和8年1月18日".to_string(),
            work_type: "道路舗装工事".to_string(),
            variety: "アスファルト表層工".to_string(),
            detail: "敷均し・締固め".to_string(),
            station: "測点No.10+5.5".to_string(),
            remarks: "天候：晴れ　気温：15℃".to_string(),
            description: String::new(),
            has_board: false,
            photo_category: "施工状況写真".to_string(),
            measurements: "厚さ50mm".to_string(),
            detected_text: String::new(),
            reasoning: String::new(),
        },
    ];

    excel::generate_excel(&results, &excel_path, "日本語テスト")
        .expect("Excel生成に失敗");

    // 読み戻し
    let mut workbook: Xlsx<_> = open_workbook(&excel_path)
        .expect("Excelファイルを開けない");

    let sheet_name = workbook.sheet_names().first().cloned().unwrap();
    let range = workbook.worksheet_range(&sheet_name).unwrap();

    // 日本語が正しく保存されているか確認
    let file_name = range.get_value((1, 0)).map(|v| v.to_string()).unwrap_or_default();
    assert_eq!(file_name, "写真001.jpg", "日本語ファイル名が壊れている");

    let work_type = range.get_value((1, 3)).map(|v| v.to_string()).unwrap_or_default();
    assert_eq!(work_type, "道路舗装工事", "日本語工種が壊れている");

    let remarks = range.get_value((1, 7)).map(|v| v.to_string()).unwrap_or_default();
    assert!(remarks.contains("晴れ"), "日本語備考が壊れている: {}", remarks);
    assert!(remarks.contains("℃"), "特殊文字が壊れている: {}", remarks);

    println!("日本語テスト: 全項目正常");
}
