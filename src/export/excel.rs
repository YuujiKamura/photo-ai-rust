//! Excel生成（CLI版）
//!
//! 共通ライブラリのexcel::generate_excel_bufferを使用

use crate::analyzer::AnalysisResult;
use crate::error::{PhotoAiError, Result};
use photo_ai_common::excel::{generate_excel_buffer, ImageData, PhotoData};
use std::path::Path;

/// CLI版AnalysisResultにPhotoDataトレイトを実装
/// （PhotoDataはphoto_ai_commonで定義、AnalysisResultはcrate内で定義なのでOK）
impl PhotoData for AnalysisResult {
    fn file_path(&self) -> &str { &self.file_path }
    fn date(&self) -> &str { &self.date }
    fn photo_category(&self) -> &str { &self.photo_category }
    fn work_type(&self) -> &str { &self.work_type }
    fn variety(&self) -> &str { &self.variety }
    fn detail(&self) -> &str { &self.detail }
    fn station(&self) -> &str { &self.station }
    fn remarks(&self) -> &str { &self.remarks }
    fn measurements(&self) -> &str { &self.measurements }
}

/// ファイルパスから画像を読み込む
fn load_image_from_file(file_path: &str) -> Option<ImageData> {
    let path = Path::new(file_path);
    if !path.exists() {
        return None;
    }

    let data = std::fs::read(path).ok()?;
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_else(|| "jpeg".to_string());

    let extension = match extension.as_str() {
        "jpg" => "jpeg".to_string(),
        other => other.to_string(),
    };

    Some(ImageData { data, extension })
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
    // 共通ライブラリを使用してExcel生成
    let buffer = generate_excel_buffer(results, photos_per_page, load_image_from_file)
        .map_err(PhotoAiError::ExcelGeneration)?;

    // ファイルに書き出し
    std::fs::write(output_path, buffer)
        .map_err(|e| PhotoAiError::ExcelGeneration(format!("ファイル書き込みエラー: {}", e)))?;

    Ok(())
}
