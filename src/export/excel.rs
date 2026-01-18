//! Excel生成（CLI版）
//!
//! layout.rs の定義を使用して写真台帳形式のExcelを生成

use crate::analyzer::AnalysisResult;
use crate::error::{PhotoAiError, Result};
use super::layout::{
    ExcelLayout, LAYOUT_FIELDS,
    PHOTO_ROWS, ROW_HEIGHT_PT,
    PHOTO_COL_WIDTH, LABEL_COL_WIDTH, VALUE_COL_WIDTH,
};
use rust_xlsxwriter::*;
use std::path::Path;

/// フィールド値を取得
fn get_field_value<'a>(result: &'a AnalysisResult, key: &str) -> &'a str {
    match key {
        "date" => if result.date.is_empty() { "-" } else { &result.date },
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
    title: &str,
) -> Result<()> {
    generate_excel_with_options(results, output_path, title, 3)
}

pub fn generate_excel_with_options(
    results: &[AnalysisResult],
    output_path: &Path,
    _title: &str,
    photos_per_page: u8,
) -> Result<()> {
    let layout = ExcelLayout::for_photos_per_page(photos_per_page);
    let mut workbook = Workbook::new();

    // フォーマット定義
    let label_format = Format::new()
        .set_bold()
        .set_font_size(9.0)
        .set_font_color(Color::RGB(0x555555))
        .set_background_color(Color::RGB(0xF5F5F5))
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Hair)
        .set_border_color(Color::RGB(0xAAAAAA));

    let value_format = Format::new()
        .set_font_size(11.0)
        .set_align(FormatAlign::Left)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap()
        .set_border(FormatBorder::Hair)
        .set_border_color(Color::RGB(0xCCCCCC));

    let photo_cell_format = Format::new()
        .set_border(FormatBorder::Thin)
        .set_border_color(Color::RGB(0xCCCCCC));

    // ページごとにシートを作成
    let total_pages = results.len().div_ceil(photos_per_page as usize);

    for page_num in 0..total_pages {
        let start_idx = page_num * photos_per_page as usize;
        let end_idx = std::cmp::min(start_idx + photos_per_page as usize, results.len());
        let page_photos = &results[start_idx..end_idx];

        let sheet_name = format!("{}", page_num + 1);
        let worksheet = workbook.add_worksheet();
        worksheet.set_name(&sheet_name)
            .map_err(|e| PhotoAiError::ExcelGeneration(format!("シート名設定エラー: {}", e)))?;

        // 列幅設定
        worksheet.set_column_width(0, PHOTO_COL_WIDTH as f64)
            .map_err(|e| PhotoAiError::ExcelGeneration(format!("列幅設定エラー: {}", e)))?;
        worksheet.set_column_width(1, LABEL_COL_WIDTH as f64)
            .map_err(|e| PhotoAiError::ExcelGeneration(format!("列幅設定エラー: {}", e)))?;
        worksheet.set_column_width(2, VALUE_COL_WIDTH as f64)
            .map_err(|e| PhotoAiError::ExcelGeneration(format!("列幅設定エラー: {}", e)))?;

        let mut current_row: u32 = 0;

        for photo in page_photos {
            let start_row = current_row;
            let rows_per_block = layout.rows_per_block as u32;
            let photo_rows = PHOTO_ROWS as u32;

            // 行高さ設定
            for r in start_row..(start_row + rows_per_block) {
                worksheet.set_row_height(r, ROW_HEIGHT_PT as f64)
                    .map_err(|e| PhotoAiError::ExcelGeneration(format!("行高さ設定エラー: {}", e)))?;
            }

            // 写真セル（A列）- マージ
            let photo_end_row = start_row + photo_rows - 1;
            worksheet.merge_range(start_row, 0, photo_end_row, 0, "", &photo_cell_format)
                .map_err(|e| PhotoAiError::ExcelGeneration(format!("セルマージエラー: {}", e)))?;

            // 画像埋め込み
            if !photo.file_path.is_empty() && Path::new(&photo.file_path).exists() {
                let image = Image::new(&photo.file_path)
                    .map_err(|e| PhotoAiError::ExcelGeneration(format!("画像読み込みエラー: {}", e)))?;

                // セル内にフィットさせる
                worksheet.insert_image_fit_to_cell(start_row, 0, &image, false)
                    .map_err(|e| PhotoAiError::ExcelGeneration(format!("画像埋め込みエラー: {}", e)))?;
            }

            // 情報フィールド（B列:ラベル、C列:値）
            let mut field_row = start_row;
            for field in LAYOUT_FIELDS.iter() {
                let value = get_field_value(photo, field.key);
                let row_span = field.row_span as u32;

                // ラベルセル（B列）
                if row_span > 1 {
                    worksheet.merge_range(field_row, 1, field_row + row_span - 1, 1, field.label, &label_format)
                        .map_err(|e| PhotoAiError::ExcelGeneration(format!("ラベルマージエラー: {}", e)))?;
                } else {
                    worksheet.write_string_with_format(field_row, 1, field.label, &label_format)
                        .map_err(|e| PhotoAiError::ExcelGeneration(format!("ラベル書き込みエラー: {}", e)))?;
                }

                // 値セル（C列）
                if row_span > 1 {
                    worksheet.merge_range(field_row, 2, field_row + row_span - 1, 2, value, &value_format)
                        .map_err(|e| PhotoAiError::ExcelGeneration(format!("値マージエラー: {}", e)))?;
                } else {
                    worksheet.write_string_with_format(field_row, 2, value, &value_format)
                        .map_err(|e| PhotoAiError::ExcelGeneration(format!("値書き込みエラー: {}", e)))?;
                }

                field_row += row_span;
            }

            current_row = start_row + rows_per_block;
        }
    }

    // 保存
    workbook.save(output_path)
        .map_err(|e| PhotoAiError::ExcelGeneration(format!("Excel保存エラー: {}", e)))?;

    Ok(())
}
