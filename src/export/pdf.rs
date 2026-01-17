//! PDF生成モジュール
//!
//! 写真台帳PDFを生成する。A4縦、1ページあたり2〜3枚の写真を配置。

use crate::analyzer::AnalysisResult;
use crate::error::{PhotoAiError, Result};
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

// re-export for image loading
use ::image as image_crate;

// A4サイズ（mm）
const A4_WIDTH_MM: f32 = 210.0;
const A4_HEIGHT_MM: f32 = 297.0;
const MARGIN_MM: f32 = 15.0;

// 写真台帳レイアウト（3枚/ページ）
const PHOTO_WIDTH_MM: f32 = 80.0;
const PHOTO_HEIGHT_MM: f32 = 60.0;
const ROW_HEIGHT_MM: f32 = 80.0;

/// 写真台帳PDFを生成
pub fn generate_pdf(
    results: &[AnalysisResult],
    output_path: &Path,
    photos_per_page: u8,
    title: &str,
) -> Result<()> {
    let photos_per_page = photos_per_page.max(2).min(3) as usize;

    let (doc, page1, layer1) = PdfDocument::new(
        title,
        Mm(A4_WIDTH_MM),
        Mm(A4_HEIGHT_MM),
        "Layer 1",
    );

    // フォント（日本語は後で対応）
    let font = doc.add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| PhotoAiError::PdfGeneration(format!("フォント追加エラー: {:?}", e)))?;

    let font_bold = doc.add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| PhotoAiError::PdfGeneration(format!("フォント追加エラー: {:?}", e)))?;

    let mut current_page = doc.get_page(page1);
    let mut current_layer = current_page.get_layer(layer1);

    // タイトル
    current_layer.use_text(
        title,
        16.0,
        Mm(A4_WIDTH_MM / 2.0 - 30.0),
        Mm(A4_HEIGHT_MM - MARGIN_MM - 5.0),
        &font_bold,
    );

    let mut photo_index = 0;
    let mut row_on_page = 0;
    let content_start_y = A4_HEIGHT_MM - MARGIN_MM - 20.0;

    for result in results {
        // 新しいページが必要かチェック
        if row_on_page >= photos_per_page {
            let (new_page, new_layer) = doc.add_page(
                Mm(A4_WIDTH_MM),
                Mm(A4_HEIGHT_MM),
                "Layer 1",
            );
            current_page = doc.get_page(new_page);
            current_layer = current_page.get_layer(new_layer);
            row_on_page = 0;
        }

        let row_y = content_start_y - (row_on_page as f32 * ROW_HEIGHT_MM);

        // 写真配置
        if !result.file_path.is_empty() {
            if let Err(e) = embed_image(&current_layer, &result.file_path, MARGIN_MM, row_y - PHOTO_HEIGHT_MM) {
                eprintln!("警告: 写真埋め込み失敗 ({}): {}", result.file_name, e);
            }
        }

        // 情報配置（写真の右側）
        let info_x = MARGIN_MM + PHOTO_WIDTH_MM + 5.0;
        let info_y = row_y - 5.0;

        // 番号
        current_layer.use_text(
            &format!("No.{}", photo_index + 1),
            10.0,
            Mm(info_x),
            Mm(info_y),
            &font_bold,
        );

        // ファイル名
        current_layer.use_text(
            &result.file_name,
            8.0,
            Mm(info_x),
            Mm(info_y - 12.0),
            &font,
        );

        // 写真区分（ASCII文字のみ出力可能）
        let category_ascii = to_ascii_placeholder(&result.photo_category);
        current_layer.use_text(
            &format!("Category: {}", category_ascii),
            8.0,
            Mm(info_x),
            Mm(info_y - 22.0),
            &font,
        );

        // 測定値
        if !result.measurements.is_empty() {
            let meas_ascii = to_ascii_placeholder(&result.measurements);
            current_layer.use_text(
                &format!("Data: {}", meas_ascii),
                8.0,
                Mm(info_x),
                Mm(info_y - 32.0),
                &font,
            );
        }

        // 区切り線
        draw_row_border(&current_layer, MARGIN_MM, row_y - ROW_HEIGHT_MM + 5.0);

        photo_index += 1;
        row_on_page += 1;
    }

    // ページ番号は各ページ作成時に追加済み（printpdf 0.7の制限により後から追加不可）

    // 保存
    let file = File::create(output_path)?;
    let writer = BufWriter::new(file);
    doc.save(&mut BufWriter::new(writer))
        .map_err(|e| PhotoAiError::PdfGeneration(format!("PDF保存エラー: {:?}", e)))?;

    Ok(())
}

/// 画像をPDFに埋め込む
fn embed_image(layer: &PdfLayerReference, image_path: &str, x_mm: f32, y_mm: f32) -> Result<()> {
    let path = Path::new(image_path);

    if !path.exists() {
        return Err(PhotoAiError::FileNotFound(image_path.to_string()));
    }

    // imageクレートで画像を読み込み
    let dynamic_image = image_crate::open(path)
        .map_err(|e| PhotoAiError::PdfGeneration(format!("画像読み込みエラー: {}", e)))?;

    // RGBに変換
    let rgb_image = dynamic_image.to_rgb8();
    let (width, height) = rgb_image.dimensions();

    // printpdf用のRawImageを作成
    let image_data = rgb_image.into_raw();
    let image = Image::from(ImageXObject {
        width: Px(width as usize),
        height: Px(height as usize),
        color_space: ColorSpace::Rgb,
        bits_per_component: ColorBits::Bit8,
        interpolate: true,
        image_data,
        image_filter: None,
        smask: None,
        clipping_bbox: None,
    });

    // アスペクト比を維持してサイズ調整
    let aspect = width as f32 / height as f32;
    let (scale_w, scale_h) = if aspect > (PHOTO_WIDTH_MM / PHOTO_HEIGHT_MM) {
        // 横長: 幅に合わせる
        let scale = PHOTO_WIDTH_MM / (width as f32 * 25.4 / 72.0);
        (scale, scale)
    } else {
        // 縦長: 高さに合わせる
        let scale = PHOTO_HEIGHT_MM / (height as f32 * 25.4 / 72.0);
        (scale, scale)
    };

    // 画像を配置
    let transform = ImageTransform {
        translate_x: Some(Mm(x_mm)),
        translate_y: Some(Mm(y_mm)),
        scale_x: Some(scale_w),
        scale_y: Some(scale_h),
        ..Default::default()
    };

    image.add_to_layer(layer.clone(), transform);

    Ok(())
}

/// 行の区切り線を描画
fn draw_row_border(layer: &PdfLayerReference, x_mm: f32, y_mm: f32) {
    let line = Line {
        points: vec![
            (Point::new(Mm(x_mm), Mm(y_mm)), false),
            (Point::new(Mm(A4_WIDTH_MM - MARGIN_MM), Mm(y_mm)), false),
        ],
        is_closed: false,
    };

    layer.set_outline_color(Color::Rgb(Rgb::new(0.7, 0.7, 0.7, None)));
    layer.set_outline_thickness(0.5);
    layer.add_line(line);
}

/// 日本語をASCIIプレースホルダーに変換（暫定）
fn to_ascii_placeholder(text: &str) -> String {
    // 日本語フォント対応まではASCII文字のみ出力
    text.chars()
        .map(|c| if c.is_ascii() { c } else { '?' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_ascii_placeholder() {
        assert_eq!(to_ascii_placeholder("Hello"), "Hello");
        assert_eq!(to_ascii_placeholder("品質管理"), "????");
        assert_eq!(to_ascii_placeholder("160.4℃"), "160.4?");
    }
}
