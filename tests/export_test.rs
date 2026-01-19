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
        subphase: format!("テスト写真{}", index),
        station: format!("No.{}+0.0", index * 10),
        remarks: "備考テスト".to_string(),
        description: format!("テスト説明{}", index),
        has_board: false,
        photo_category: "施工状況".to_string(),
        measurements: "50mm".to_string(),
        detected_text: String::new(),
        reasoning: String::new(),
        remarks_candidates: Vec::new(),
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

/// Excelに書き込んだデータが正しく読み戻せることを確認（写真台帳形式）
///
/// 写真台帳形式のレイアウト:
/// - A列(0): 写真セル（マージ）
/// - B列(1): ラベル（日時、区分、工種、種別、作業段階、測点、備考、測定値）
/// - C列(2): 値
/// - 各写真ブロックは11行（写真10行 + ギャップ1行）
#[test]
fn test_excel_data_consistency() {
    let dir = tempdir().expect("Failed to create temp dir");
    let excel_path = dir.path().join("data_check.xlsx");

    // テストデータ作成
    let results = vec![
        AnalysisResult {
            file_name: "IMG_001.jpg".to_string(),
            file_path: String::new(),
            date: "2026-01-15".to_string(),
            work_type: "舗装工事".to_string(),
            variety: "表層工".to_string(),
            subphase: "アスファルト舗設".to_string(),
            station: "No.5+10.0".to_string(),
            remarks: "1層目".to_string(),
            description: String::new(),
            has_board: false,
            photo_category: "施工状況".to_string(),
            measurements: "t=50mm".to_string(),
            detected_text: String::new(),
            reasoning: String::new(),
            remarks_candidates: Vec::new(),
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

    // 写真台帳形式: B列にラベル、C列に値
    // 行0: 日時
    let label_date = range.get_value((0, 1)).map(|v| v.to_string()).unwrap_or_default();
    assert_eq!(label_date, "日時", "ラベル「日時」がない");

    let value_date = range.get_value((0, 2)).map(|v| v.to_string()).unwrap_or_default();
    assert_eq!(value_date, "2026-01-15", "日付値が一致しない");

    // 行2: 工種
    let label_work = range.get_value((2, 1)).map(|v| v.to_string()).unwrap_or_default();
    assert_eq!(label_work, "工種", "ラベル「工種」がない");

    let value_work = range.get_value((2, 2)).map(|v| v.to_string()).unwrap_or_default();
    assert_eq!(value_work, "舗装工事", "工種値が一致しない");

    // 行5: 測点
    let label_station = range.get_value((5, 1)).map(|v| v.to_string()).unwrap_or_default();
    assert_eq!(label_station, "測点", "ラベル「測点」がない");

    let value_station = range.get_value((5, 2)).map(|v| v.to_string()).unwrap_or_default();
    assert_eq!(value_station, "No.5+10.0", "測点値が一致しない");

    println!("Excel整合性検証（写真台帳形式）: 全項目一致");
}

/// 日本語文字が正しく保存・読み戻しされることを確認（写真台帳形式）
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
            subphase: "敷均し・締固め".to_string(),
            station: "測点No.10+5.5".to_string(),
            remarks: "天候：晴れ　気温：15℃".to_string(),
            description: String::new(),
            has_board: false,
            photo_category: "施工状況写真".to_string(),
            measurements: "厚さ50mm".to_string(),
            detected_text: String::new(),
            reasoning: String::new(),
            remarks_candidates: Vec::new(),
        },
    ];

    excel::generate_excel(&results, &excel_path, "日本語テスト")
        .expect("Excel生成に失敗");

    // 読み戻し
    let mut workbook: Xlsx<_> = open_workbook(&excel_path)
        .expect("Excelファイルを開けない");

    let sheet_name = workbook.sheet_names().first().cloned().unwrap();
    let range = workbook.worksheet_range(&sheet_name).unwrap();

    // 写真台帳形式: B列(1)=ラベル、C列(2)=値
    // 行0: 日時
    let value_date = range.get_value((0, 2)).map(|v| v.to_string()).unwrap_or_default();
    assert_eq!(value_date, "令和8年1月18日", "日本語日付が壊れている");

    // 行2: 工種
    let value_work = range.get_value((2, 2)).map(|v| v.to_string()).unwrap_or_default();
    assert_eq!(value_work, "道路舗装工事", "日本語工種が壊れている");

    // 行6: 備考（row_span=2なのでマージセル、calamine読み取り時は先頭行に値）
    let value_remarks = range.get_value((6, 2)).map(|v| v.to_string()).unwrap_or_default();
    assert!(value_remarks.contains("晴れ"), "日本語備考が壊れている: {}", value_remarks);
    assert!(value_remarks.contains("℃"), "特殊文字が壊れている: {}", value_remarks);

    // 行8: 測定値（row_span=3）
    let value_measurements = range.get_value((8, 2)).map(|v| v.to_string()).unwrap_or_default();
    assert_eq!(value_measurements, "厚さ50mm", "日本語測定値が壊れている");

    println!("日本語テスト（写真台帳形式）: 全項目正常");
}
