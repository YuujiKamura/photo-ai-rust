use crate::analyzer::AnalysisResult;
use crate::error::{PhotoAiError, Result};
use super::layout::{LAYOUT_FIELDS, LABEL_COL_WIDTH, VALUE_COL_WIDTH};
use rust_xlsxwriter::*;
use std::path::Path;

/// フィールド値を取得（LAYOUT_FIELDSのkeyに基づく）
fn get_field_value<'a>(result: &'a AnalysisResult, key: &str) -> &'a str {
    match key {
        "date" => "-",  // TODO: EXIF日時実装後に対応
        "photoCategory" => &result.photo_category,
        "workType" => &result.work_type,
        "variety" => &result.variety,
        "detail" => &result.detail,
        "station" => &result.station,
        "remarks" => &result.remarks,
        "measurements" => &result.measurements,
        _ => "-",
    }
}

pub fn generate_excel(
    results: &[AnalysisResult],
    output_path: &Path,
    _title: &str,
) -> Result<()> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    // タイトル行のフォーマット
    let header_format = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xE0E0E0))
        .set_border(FormatBorder::Thin);

    // ヘッダー: ファイル名 + LAYOUT_FIELDSのラベル
    worksheet.write_string_with_format(0, 0, "ファイル名", &header_format)
        .map_err(|e| PhotoAiError::ExcelGeneration(format!("ヘッダー書き込みエラー: {}", e)))?;

    for (col, field) in LAYOUT_FIELDS.iter().enumerate() {
        worksheet.write_string_with_format(0, (col + 1) as u16, field.label, &header_format)
            .map_err(|e| PhotoAiError::ExcelGeneration(format!("ヘッダー書き込みエラー: {}", e)))?;
    }

    // データ行
    for (row, result) in results.iter().enumerate() {
        let row_num = (row + 1) as u32;

        // ファイル名
        worksheet.write_string(row_num, 0, &result.file_name)
            .map_err(|e| PhotoAiError::ExcelGeneration(format!("データ書き込みエラー: {}", e)))?;

        // LAYOUT_FIELDSの各フィールド
        for (col, field) in LAYOUT_FIELDS.iter().enumerate() {
            let value = get_field_value(result, field.key);
            worksheet.write_string(row_num, (col + 1) as u16, value)
                .map_err(|e| PhotoAiError::ExcelGeneration(format!("データ書き込みエラー: {}", e)))?;
        }
    }

    // 列幅調整（layout.rsの定数を使用）
    worksheet.set_column_width(0, 20.0)  // ファイル名
        .map_err(|e| PhotoAiError::ExcelGeneration(format!("列幅設定エラー: {}", e)))?;

    // ラベル列とデータ列の幅
    for col in 1..=(LAYOUT_FIELDS.len() as u16) {
        let width = if col % 2 == 1 { LABEL_COL_WIDTH as f64 } else { VALUE_COL_WIDTH as f64 };
        worksheet.set_column_width(col, width)
            .map_err(|e| PhotoAiError::ExcelGeneration(format!("列幅設定エラー: {}", e)))?;
    }

    // 保存
    workbook.save(output_path)
        .map_err(|e| PhotoAiError::ExcelGeneration(format!("Excel保存エラー: {}", e)))?;

    Ok(())
}
