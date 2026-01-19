//! Excel生成（共通ライブラリ）
//!
//! layout.rs の定義を使用して写真台帳形式のExcelを生成

use crate::layout::{
    excel_width_to_px, PT_TO_PX,
    ExcelLayout, LAYOUT_FIELDS,
    PHOTO_COL_WIDTH, PHOTO_ROWS,
    LABEL_COL_WIDTH, VALUE_COL_WIDTH,
};
use rust_xlsxwriter::*;

/// 画像データ（バイト配列）
pub struct ImageData {
    pub data: Vec<u8>,
    pub extension: String,  // "png", "jpeg", "gif"
}

/// 写真データのトレイト（異なるAnalysisResult型に対応）
pub trait PhotoData {
    fn file_path(&self) -> &str;
    fn date(&self) -> &str;
    fn photo_category(&self) -> &str;
    fn work_type(&self) -> &str;
    fn variety(&self) -> &str;
    fn subphase(&self) -> &str;
    fn station(&self) -> &str;
    fn remarks(&self) -> &str;
    fn measurements(&self) -> &str;
}

/// common::AnalysisResultにPhotoDataを実装
impl PhotoData for crate::types::AnalysisResult {
    fn file_path(&self) -> &str { &self.file_path }
    fn date(&self) -> &str { &self.date }
    fn photo_category(&self) -> &str { &self.photo_category }
    fn work_type(&self) -> &str { &self.work_type }
    fn variety(&self) -> &str { &self.variety }
    fn subphase(&self) -> &str { &self.subphase }
    fn station(&self) -> &str { &self.station }
    fn remarks(&self) -> &str { &self.remarks }
    fn measurements(&self) -> &str { &self.measurements }
}

/// フィールド値を取得
fn get_field_value<'a, T: PhotoData>(data: &'a T, key: &str) -> &'a str {
    match key {
        "date" => {
            let d = data.date();
            if d.is_empty() { "-" } else { d }
        },
        "photoCategory" => data.photo_category(),
        "workType" => data.work_type(),
        "variety" => data.variety(),
        "subphase" => data.subphase(),
        "station" => data.station(),
        "remarks" => data.remarks(),
        "measurements" => data.measurements(),
        _ => "-",
    }
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
) -> Result<Vec<u8>, String>
where
    T: PhotoData,
    F: Fn(&str) -> Option<ImageData>,
{
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
            .map_err(|e| format!("シート名設定エラー: {}", e))?;

        // 列幅設定（React版と同じpx換算を使用）
        let row_height_px = (layout.row_height_pt * PT_TO_PX).round() as u32;
        let col_a_px = excel_width_to_px(PHOTO_COL_WIDTH) as u32;
        let col_b_px = excel_width_to_px(LABEL_COL_WIDTH) as u32;
        let col_c_px = excel_width_to_px(VALUE_COL_WIDTH) as u32;
        worksheet.set_column_width_pixels(0, col_a_px)
            .map_err(|e| format!("列幅設定エラー: {}", e))?;
        worksheet.set_column_width_pixels(1, col_b_px)
            .map_err(|e| format!("列幅設定エラー: {}", e))?;
        worksheet.set_column_width_pixels(2, col_c_px)
            .map_err(|e| format!("列幅設定エラー: {}", e))?;

        let mut current_row: u32 = 0;

        for photo in page_photos {
            let start_row = current_row;
            let rows_per_block = layout.rows_per_block as u32;
            let photo_rows = PHOTO_ROWS as u32;

            // 行高さ設定
            for r in start_row..(start_row + rows_per_block) {
                worksheet.set_row_height_pixels(r, row_height_px)
                    .map_err(|e| format!("行高さ設定エラー: {}", e))?;
            }

            // 写真セル（A列）- マージ
            let photo_end_row = start_row + photo_rows - 1;
            worksheet.merge_range(start_row, 0, photo_end_row, 0, "", &photo_cell_format)
                .map_err(|e| format!("セルマージエラー: {}", e))?;

            // 画像埋め込み
            if let Some(image_data) = image_loader(photo.file_path()) {
                let image = Image::new_from_buffer(&image_data.data)
                    .map_err(|e| format!("画像読み込みエラー: {}", e))?;

                // 単一倍率で縦横比を維持しつつ枠内に収める
                let image = image
                    .set_scale_width(0.36)
                    .set_scale_height(0.36)
                    .set_object_movement(ObjectMovement::DontMoveOrSizeWithCells);

                worksheet.insert_image_with_offset(start_row, 0, &image, 0, 0)
                    .map_err(|e| format!("画像埋め込みエラー: {}", e))?;
            }

            // 情報フィールド（B列:ラベル、C列:値）
            let mut field_row = start_row;
            for field in LAYOUT_FIELDS.iter() {
                let value = get_field_value(photo, field.key);
                let row_span = field.row_span as u32;

                // ラベルセル（B列）
                if row_span > 1 {
                    worksheet.merge_range(field_row, 1, field_row + row_span - 1, 1, field.label, &label_format)
                        .map_err(|e| format!("ラベルマージエラー: {}", e))?;
                } else {
                    worksheet.write_string_with_format(field_row, 1, field.label, &label_format)
                        .map_err(|e| format!("ラベル書き込みエラー: {}", e))?;
                }

                // 値セル（C列）
                if row_span > 1 {
                    worksheet.merge_range(field_row, 2, field_row + row_span - 1, 2, value, &value_format)
                        .map_err(|e| format!("値マージエラー: {}", e))?;
                } else {
                    worksheet.write_string_with_format(field_row, 2, value, &value_format)
                        .map_err(|e| format!("値書き込みエラー: {}", e))?;
                }

                field_row += row_span;
            }

            current_row = start_row + rows_per_block;
        }
    }

    // バッファに書き出し
    workbook.save_to_buffer()
        .map_err(|e| format!("Excel保存エラー: {}", e))
}
