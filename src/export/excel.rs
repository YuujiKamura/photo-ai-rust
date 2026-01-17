use crate::analyzer::AnalysisResult;
use crate::error::{PhotoAiError, Result};
use rust_xlsxwriter::*;
use std::path::Path;

pub fn generate_excel(
    results: &[AnalysisResult],
    output_path: &Path,
    title: &str,
) -> Result<()> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    // タイトル行のフォーマット
    let header_format = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xE0E0E0))
        .set_border(FormatBorder::Thin);

    // ヘッダー
    let headers = [
        "ファイル名",
        "工種",
        "種別",
        "細別",
        "測点",
        "備考",
        "写真説明",
        "写真区分",
    ];

    for (col, header) in headers.iter().enumerate() {
        worksheet.write_string_with_format(0, col as u16, *header, &header_format)
            .map_err(|e| PhotoAiError::ExcelGeneration(format!("ヘッダー書き込みエラー: {}", e)))?;
    }

    // データ行
    for (row, result) in results.iter().enumerate() {
        let row_num = (row + 1) as u32;

        worksheet.write_string(row_num, 0, &result.file_name)
            .map_err(|e| PhotoAiError::ExcelGeneration(format!("データ書き込みエラー: {}", e)))?;
        worksheet.write_string(row_num, 1, &result.work_type)
            .map_err(|e| PhotoAiError::ExcelGeneration(format!("データ書き込みエラー: {}", e)))?;
        worksheet.write_string(row_num, 2, &result.variety)
            .map_err(|e| PhotoAiError::ExcelGeneration(format!("データ書き込みエラー: {}", e)))?;
        worksheet.write_string(row_num, 3, &result.detail)
            .map_err(|e| PhotoAiError::ExcelGeneration(format!("データ書き込みエラー: {}", e)))?;
        worksheet.write_string(row_num, 4, &result.station)
            .map_err(|e| PhotoAiError::ExcelGeneration(format!("データ書き込みエラー: {}", e)))?;
        worksheet.write_string(row_num, 5, &result.remarks)
            .map_err(|e| PhotoAiError::ExcelGeneration(format!("データ書き込みエラー: {}", e)))?;
        worksheet.write_string(row_num, 6, &result.description)
            .map_err(|e| PhotoAiError::ExcelGeneration(format!("データ書き込みエラー: {}", e)))?;
        worksheet.write_string(row_num, 7, &result.photo_category)
            .map_err(|e| PhotoAiError::ExcelGeneration(format!("データ書き込みエラー: {}", e)))?;
    }

    // 列幅調整
    worksheet.set_column_width(0, 20.0)
        .map_err(|e| PhotoAiError::ExcelGeneration(format!("列幅設定エラー: {}", e)))?;
    worksheet.set_column_width(6, 40.0)
        .map_err(|e| PhotoAiError::ExcelGeneration(format!("列幅設定エラー: {}", e)))?;

    // 保存
    workbook.save(output_path)
        .map_err(|e| PhotoAiError::ExcelGeneration(format!("Excel保存エラー: {}", e)))?;

    Ok(())
}
