//! Excel生成（共通ライブラリ）
//!
//! layout.rs の定義を使用して写真台帳形式のExcelを生成

use crate::error::Error;
use super::ExportError;
use crate::layout::{
    excel_width_to_px, fit_image_centered, PT_TO_PX,
    ExcelLayout, LAYOUT_FIELDS,
    LABEL_COL_WIDTH, VALUE_COL_WIDTH,
    LABEL_FONT_SIZE, LABEL_FONT_COLOR, LABEL_BG_COLOR, LABEL_BORDER_COLOR,
    VALUE_FONT_SIZE, CELL_BORDER_COLOR,
};
use rust_xlsxwriter::*;

pub use crate::types::PhotoData;
pub use super::ImageData;

/// ラベルセル用フォーマット
fn create_label_format() -> Format {
    Format::new()
        .set_bold()
        .set_font_size(LABEL_FONT_SIZE)
        .set_font_color(Color::RGB(LABEL_FONT_COLOR))
        .set_background_color(Color::RGB(LABEL_BG_COLOR))
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Hair)
        .set_border_color(Color::RGB(LABEL_BORDER_COLOR))
}

/// 値セル用フォーマット
fn create_value_format() -> Format {
    Format::new()
        .set_font_size(VALUE_FONT_SIZE)
        .set_align(FormatAlign::Left)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap()
        .set_border(FormatBorder::Hair)
        .set_border_color(Color::RGB(CELL_BORDER_COLOR))
}

/// 写真セル用フォーマット
fn create_photo_cell_format() -> Format {
    Format::new()
        .set_border(FormatBorder::Thin)
        .set_border_color(Color::RGB(CELL_BORDER_COLOR))
}

/// Excelをバッファに生成
///
/// # Arguments
/// * `results` - 写真データ（PhotoDataトレイトを実装した型）
/// * `photos_per_page` - 1ページあたりの写真数
/// * `image_loader` - 画像データを取得するクロージャ (file_path -> Option<ImageData>)
pub fn generate_excel_buffer<T, F>(
    results: &[T],
    photos_per_page: u8,
    image_loader: F,
) -> Result<Vec<u8>, Error>
where
    T: PhotoData,
    F: Fn(&str) -> Option<ImageData>,
{
    let excel_err = |detail: String| Error::from(ExportError::Failed {
        format: "Excel".to_string(),
        detail,
    });

    let layout = ExcelLayout::for_photos_per_page(photos_per_page);
    let mut workbook = Workbook::new();

    // フォーマット定義
    let label_format = create_label_format();
    let value_format = create_value_format();
    let photo_cell_format = create_photo_cell_format();

    // ページごとにシートを作成
    let total_pages = results.len().div_ceil(photos_per_page as usize);

    for page_num in 0..total_pages {
        let start_idx = page_num * photos_per_page as usize;
        let end_idx = std::cmp::min(start_idx + photos_per_page as usize, results.len());
        let page_photos = &results[start_idx..end_idx];

        let sheet_name = format!("{}", page_num + 1);
        let worksheet = workbook.add_worksheet();
        worksheet.set_name(&sheet_name)
            .map_err(|e| excel_err(format!("シート名設定: {}", e)))?;

        // 列幅設定（React版と同じpx換算を使用）
        let (col_a_px, target_height_px) = layout.photo_area_px();
        let row_height_px = (layout.row_height_pt * PT_TO_PX).round() as u32;
        let col_b_px = excel_width_to_px(LABEL_COL_WIDTH) as u32;
        let col_c_px = excel_width_to_px(VALUE_COL_WIDTH) as u32;
        worksheet.set_column_width_pixels(0, col_a_px)
            .map_err(|e| excel_err(format!("列幅設定: {}", e)))?;
        worksheet.set_column_width_pixels(1, col_b_px)
            .map_err(|e| excel_err(format!("列幅設定: {}", e)))?;
        worksheet.set_column_width_pixels(2, col_c_px)
            .map_err(|e| excel_err(format!("列幅設定: {}", e)))?;

        let mut current_row: u32 = 0;

        for photo in page_photos {
            let start_row = current_row;
            let rows_per_block = layout.rows_per_block as u32;
            let photo_rows = layout.photo_rows as u32;

            // 行高さ設定
            for r in start_row..(start_row + rows_per_block) {
                worksheet.set_row_height_pixels(r, row_height_px)
                    .map_err(|e| excel_err(format!("行高さ設定: {}", e)))?;
            }

            // 写真セル（A列）- マージ
            let photo_end_row = start_row + photo_rows - 1;
            worksheet.merge_range(start_row, 0, photo_end_row, 0, "", &photo_cell_format)
                .map_err(|e| excel_err(format!("セルマージ: {}", e)))?;

            // 画像埋め込み
            if let Some(image_data) = image_loader(photo.file_path()) {
                let image = Image::new_from_buffer(&image_data.data)
                    .map_err(|e| excel_err(format!("画像読み込み: {}", e)))?;

                // アスペクト比を維持しつつ枠内にフィット＋中央配置
                let (_, _, off_x, off_y) = fit_image_centered(
                    image.width(),
                    image.height(),
                    col_a_px as f64,
                    target_height_px as f64,
                );
                let k = (col_a_px as f64 / image.width())
                    .min(target_height_px as f64 / image.height());
                let image = image
                    .set_scale_width(k)
                    .set_scale_height(k)
                    .set_object_movement(ObjectMovement::DontMoveOrSizeWithCells);

                worksheet.insert_image_with_offset(
                    start_row, 0, &image,
                    off_x.round() as u32,
                    off_y.round() as u32,
                )
                    .map_err(|e| excel_err(format!("画像埋め込み: {}", e)))?;
            }

            // 情報フィールド（B列:ラベル、C列:値）
            let mut field_row = start_row;
            for field in LAYOUT_FIELDS.iter() {
                let value = photo.get_field_value(field.key);
                let row_span = field.row_span as u32;

                // ラベルセル（B列）
                if row_span > 1 {
                    worksheet.merge_range(field_row, 1, field_row + row_span - 1, 1, field.label, &label_format)
                        .map_err(|e| excel_err(format!("ラベルマージ: {}", e)))?;
                } else {
                    worksheet.write_string_with_format(field_row, 1, field.label, &label_format)
                        .map_err(|e| excel_err(format!("ラベル書き込み: {}", e)))?;
                }

                // 値セル（C列）
                if row_span > 1 {
                    worksheet.merge_range(field_row, 2, field_row + row_span - 1, 2, value, &value_format)
                        .map_err(|e| excel_err(format!("値マージ: {}", e)))?;
                } else {
                    worksheet.write_string_with_format(field_row, 2, value, &value_format)
                        .map_err(|e| excel_err(format!("値書き込み: {}", e)))?;
                }

                field_row += row_span;
            }

            current_row = start_row + rows_per_block;
        }
    }

    // バッファに書き出し
    workbook.save_to_buffer()
        .map_err(|e| excel_err(format!("保存: {}", e)))
}
