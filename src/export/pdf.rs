//! PDF生成モジュール
//!
//! 写真台帳PDFを生成する。A4縦、1ページあたり2〜3枚の写真を配置。
//! 日本語フォント（メイリオ等）を自動検出して使用。

use crate::analyzer::AnalysisResult;
use crate::error::{PhotoAiError, Result};
use printpdf::*;
use std::fs::File;
use std::io::{BufWriter, Cursor};
use std::path::{Path, PathBuf};

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

/// フォント情報を保持
struct FontSet {
    regular: IndirectFontRef,
    bold: IndirectFontRef,
    is_japanese: bool,
}

/// 日本語フォントのパスを検索
fn find_japanese_font() -> Option<PathBuf> {
    // Windows
    #[cfg(windows)]
    {
        let windows_fonts = Path::new("C:\\Windows\\Fonts");
        // 優先順: メイリオ > 游ゴシック > MSゴシック
        let candidates = [
            "meiryo.ttc",
            "YuGothM.ttc",
            "YuGothR.ttc",
            "msgothic.ttc",
        ];
        for font in candidates {
            let path = windows_fonts.join(font);
            if path.exists() {
                return Some(path);
            }
        }
    }

    // macOS
    #[cfg(target_os = "macos")]
    {
        let paths = [
            "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/Library/Fonts/Arial Unicode.ttf",
        ];
        for p in paths {
            let path = Path::new(p);
            if path.exists() {
                return Some(path.to_path_buf());
            }
        }
    }

    // Linux
    #[cfg(target_os = "linux")]
    {
        let paths = [
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/takao-gothic/TakaoGothic.ttf",
        ];
        for p in paths {
            let path = Path::new(p);
            if path.exists() {
                return Some(path.to_path_buf());
            }
        }
    }

    None
}

/// フォントをロード（日本語フォントがあれば優先）
fn load_fonts(doc: &PdfDocumentReference) -> Result<FontSet> {
    // 日本語フォントを検索
    if let Some(font_path) = find_japanese_font() {
        match load_ttf_font(doc, &font_path) {
            Ok(font) => {
                eprintln!("日本語フォント使用: {}", font_path.display());
                return Ok(FontSet {
                    regular: font.clone(),
                    bold: font, // TTCは通常Regularのみ、Boldも同じに
                    is_japanese: true,
                });
            }
            Err(e) => {
                eprintln!("警告: 日本語フォント読み込み失敗: {} - フォールバック使用", e);
            }
        }
    }

    // フォールバック: 組み込みフォント
    let regular = doc.add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| PhotoAiError::PdfGeneration(format!("フォント追加エラー: {:?}", e)))?;
    let bold = doc.add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| PhotoAiError::PdfGeneration(format!("フォント追加エラー: {:?}", e)))?;

    Ok(FontSet {
        regular,
        bold,
        is_japanese: false,
    })
}

/// TTFフォントをロード
fn load_ttf_font(doc: &PdfDocumentReference, font_path: &Path) -> Result<IndirectFontRef> {
    let font_data = std::fs::read(font_path)?;
    let cursor = Cursor::new(font_data);

    let font = doc.add_external_font(cursor)
        .map_err(|e| PhotoAiError::PdfGeneration(format!("フォント埋め込みエラー: {:?}", e)))?;

    Ok(font)
}

/// テキストを処理（日本語フォントがなければASCII化）
fn process_text(text: &str, is_japanese_font: bool) -> String {
    if is_japanese_font {
        text.to_string()
    } else {
        // ASCII文字のみ（日本語は?に置換）
        text.chars()
            .map(|c| if c.is_ascii() { c } else { '?' })
            .collect()
    }
}

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

    // フォントをロード
    let fonts = load_fonts(&doc)?;

    let mut current_page = doc.get_page(page1);
    let mut current_layer = current_page.get_layer(layer1);

    // タイトル
    let title_text = process_text(title, fonts.is_japanese);
    current_layer.use_text(
        &title_text,
        16.0,
        Mm(A4_WIDTH_MM / 2.0 - 30.0),
        Mm(A4_HEIGHT_MM - MARGIN_MM - 5.0),
        &fonts.bold,
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
            &fonts.bold,
        );

        // ファイル名
        current_layer.use_text(
            &result.file_name,
            8.0,
            Mm(info_x),
            Mm(info_y - 12.0),
            &fonts.regular,
        );

        // 写真区分
        let category_label = process_text("写真区分: ", fonts.is_japanese);
        let category_value = process_text(&result.photo_category, fonts.is_japanese);
        current_layer.use_text(
            &format!("{}{}", category_label, category_value),
            8.0,
            Mm(info_x),
            Mm(info_y - 22.0),
            &fonts.regular,
        );

        // 工種・種別・細別
        if !result.work_type.is_empty() {
            let work_label = process_text("工種: ", fonts.is_japanese);
            let work_value = process_text(&result.work_type, fonts.is_japanese);
            current_layer.use_text(
                &format!("{}{}", work_label, work_value),
                8.0,
                Mm(info_x),
                Mm(info_y - 32.0),
                &fonts.regular,
            );
        }

        if !result.variety.is_empty() {
            let variety_label = process_text("種別: ", fonts.is_japanese);
            let variety_value = process_text(&result.variety, fonts.is_japanese);
            current_layer.use_text(
                &format!("{}{}", variety_label, variety_value),
                8.0,
                Mm(info_x),
                Mm(info_y - 42.0),
                &fonts.regular,
            );
        }

        // 測定値
        if !result.measurements.is_empty() {
            let meas_label = process_text("測定値: ", fonts.is_japanese);
            let meas_value = process_text(&result.measurements, fonts.is_japanese);
            current_layer.use_text(
                &format!("{}{}", meas_label, meas_value),
                8.0,
                Mm(info_x),
                Mm(info_y - 52.0),
                &fonts.regular,
            );
        }

        // 区切り線
        draw_row_border(&current_layer, MARGIN_MM, row_y - ROW_HEIGHT_MM + 5.0);

        photo_index += 1;
        row_on_page += 1;
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_text_with_japanese_font() {
        assert_eq!(process_text("品質管理", true), "品質管理");
        assert_eq!(process_text("160.4℃", true), "160.4℃");
    }

    #[test]
    fn test_process_text_without_japanese_font() {
        assert_eq!(process_text("Hello", false), "Hello");
        assert_eq!(process_text("品質管理", false), "????");
        assert_eq!(process_text("160.4℃", false), "160.4?");
    }

    #[test]
    fn test_find_japanese_font() {
        // This test just ensures the function doesn't panic
        let _result = find_japanese_font();
    }
}
