//! PDF生成モジュール
//!
//! React版 pdfGenerator.ts のロジックをそのまま移植。

use crate::analyzer::AnalysisResult;
use crate::error::{PhotoAiError, Result};
use super::layout::{self, mm_to_pt};
use printpdf::*;
use std::fs::File;
use std::io::{BufWriter, Cursor};
use std::path::{Path, PathBuf};

use ::image as image_crate;

/// フォント情報
struct FontSet {
    regular: IndirectFontRef,
    bold: IndirectFontRef,
    is_japanese: bool,
}

/// 日本語フォントのパスを検索
fn find_japanese_font() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let windows_fonts = Path::new("C:\\Windows\\Fonts");
        for font in ["meiryo.ttc", "YuGothM.ttc", "msgothic.ttc"] {
            let path = windows_fonts.join(font);
            if path.exists() {
                return Some(path);
            }
        }
    }
    None
}

/// フォントをロード
fn load_fonts(doc: &PdfDocumentReference) -> Result<FontSet> {
    if let Some(font_path) = find_japanese_font() {
        if let Ok(font) = load_ttf_font(doc, &font_path) {
            eprintln!("日本語フォント使用: {}", font_path.display());
            return Ok(FontSet {
                regular: font.clone(),
                bold: font,
                is_japanese: true,
            });
        }
    }

    let regular = doc.add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| PhotoAiError::PdfGeneration(format!("{:?}", e)))?;
    let bold = doc.add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| PhotoAiError::PdfGeneration(format!("{:?}", e)))?;

    Ok(FontSet { regular, bold, is_japanese: false })
}

fn load_ttf_font(doc: &PdfDocumentReference, font_path: &Path) -> Result<IndirectFontRef> {
    let font_data = std::fs::read(font_path)?;
    doc.add_external_font(Cursor::new(font_data))
        .map_err(|e| PhotoAiError::PdfGeneration(format!("{:?}", e)))
}

fn process_text(text: &str, is_japanese: bool) -> String {
    if is_japanese {
        text.to_string()
    } else {
        text.chars().map(|c| if c.is_ascii() { c } else { '?' }).collect()
    }
}

/// pt → mm 変換ヘルパー
fn pt_to_mm(pt: f32) -> Mm {
    Mm(pt / layout::MM_TO_PT)
}

/// 写真台帳PDFを生成（React版pdfGenerator.tsと同一ロジック）
pub fn generate_pdf(
    results: &[AnalysisResult],
    output_path: &Path,
    photos_per_page: u8,
    title: &str,
) -> Result<()> {
    let photos_per_page = photos_per_page.max(2).min(3) as usize;

    // ========== React版と同一の定数（pt単位） ==========
    let a4_width_pt = mm_to_pt(layout::A4_WIDTH_MM);    // 595.35pt
    let a4_height_pt = mm_to_pt(layout::A4_HEIGHT_MM);  // 842.0pt
    let margin_pt = mm_to_pt(layout::MARGIN_MM);        // 28.35pt
    let header_height_pt: f32 = 40.0;                   // React版: 40pt
    let photo_info_gap_pt: f32 = 5.0;                   // React版: 5pt

    // ========== React版117-122行のロジックそのまま ==========
    let usable_height_pt = a4_height_pt - margin_pt * 2.0 - header_height_pt;
    let photo_row_height_pt = usable_height_pt / photos_per_page as f32;
    let photo_height_pt = photo_row_height_pt - photo_info_gap_pt * 2.0;
    let usable_width_pt = a4_width_pt - margin_pt * 2.0;
    let photo_width_pt = usable_width_pt * layout::IMAGE_RATIO;  // 65%
    let info_width_pt = usable_width_pt * layout::INFO_RATIO;    // 35%

    let (doc, page1, layer1) = PdfDocument::new(
        title,
        Mm(layout::A4_WIDTH_MM),
        Mm(layout::A4_HEIGHT_MM),
        "Layer 1",
    );

    let fonts = load_fonts(&doc)?;
    let mut current_page = doc.get_page(page1);
    let mut current_layer = current_page.get_layer(layer1);
    let total_pages = (results.len() + photos_per_page - 1) / photos_per_page;

    // ヘッダー描画
    draw_header(&current_layer, title, 1, total_pages, &fonts, a4_width_pt, a4_height_pt, margin_pt);

    for (idx, result) in results.iter().enumerate() {
        let page_num = idx / photos_per_page;
        let slot = idx % photos_per_page;

        // 新ページ
        if idx > 0 && slot == 0 {
            let (new_page, new_layer) = doc.add_page(
                Mm(layout::A4_WIDTH_MM),
                Mm(layout::A4_HEIGHT_MM),
                "Layer 1",
            );
            current_page = doc.get_page(new_page);
            current_layer = current_page.get_layer(new_layer);
            draw_header(&current_layer, title, page_num + 1, total_pages, &fonts, a4_width_pt, a4_height_pt, margin_pt);
        }

        // ========== React版135行のY座標計算そのまま ==========
        let row_y_pt = a4_height_pt - margin_pt - header_height_pt
                     - ((slot + 1) as f32 * photo_row_height_pt)
                     + photo_info_gap_pt;

        // 写真埋め込み
        if !result.file_path.is_empty() {
            if let Err(e) = embed_image_react_style(
                &current_layer,
                &result.file_path,
                margin_pt,
                row_y_pt,
                photo_width_pt,
                photo_height_pt,
            ) {
                eprintln!("警告: 写真埋め込み失敗 ({}): {}", result.file_name, e);
            }
        }

        // 写真枠線
        draw_rect(&current_layer, margin_pt, row_y_pt, photo_width_pt, photo_height_pt);

        // ========== React版166行の情報欄位置 ==========
        let info_x_pt = margin_pt + photo_width_pt + photo_info_gap_pt;

        // 情報欄枠線
        draw_rect(&current_layer, info_x_pt, row_y_pt, info_width_pt, photo_height_pt);

        // 情報欄テキスト（React版169-185行）
        draw_info_fields(
            &current_layer,
            result,
            info_x_pt,
            row_y_pt,
            photo_height_pt,
            &fonts,
        );
    }

    // 保存
    let file = File::create(output_path)?;
    doc.save(&mut BufWriter::new(BufWriter::new(file)))
        .map_err(|e| PhotoAiError::PdfGeneration(format!("{:?}", e)))?;

    Ok(())
}

/// ヘッダー描画
fn draw_header(
    layer: &PdfLayerReference,
    title: &str,
    page_num: usize,
    total_pages: usize,
    fonts: &FontSet,
    a4_width_pt: f32,
    a4_height_pt: f32,
    margin_pt: f32,
) {
    let title_text = process_text(title, fonts.is_japanese);
    layer.use_text(
        &title_text,
        14.0,
        pt_to_mm(margin_pt),
        pt_to_mm(a4_height_pt - margin_pt - 20.0),
        &fonts.bold,
    );

    layer.use_text(
        &format!("Page {} / {}", page_num, total_pages),
        10.0,
        pt_to_mm(a4_width_pt - margin_pt - 80.0),
        pt_to_mm(a4_height_pt - margin_pt - 20.0),
        &fonts.regular,
    );
}

/// 画像埋め込み（React版146-157行のロジック）
fn embed_image_react_style(
    layer: &PdfLayerReference,
    image_path: &str,
    x_pt: f32,
    y_pt: f32,
    box_width_pt: f32,
    box_height_pt: f32,
) -> Result<()> {
    let path = Path::new(image_path);
    if !path.exists() {
        return Err(PhotoAiError::FileNotFound(image_path.to_string()));
    }

    let dynamic_image = image_crate::open(path)
        .map_err(|e| PhotoAiError::PdfGeneration(format!("画像読み込みエラー: {}", e)))?;
    let rgb_image = dynamic_image.to_rgb8();
    let (width_px, height_px) = rgb_image.dimensions();

    let image = Image::from(ImageXObject {
        width: Px(width_px as usize),
        height: Px(height_px as usize),
        color_space: ColorSpace::Rgb,
        bits_per_component: ColorBits::Bit8,
        interpolate: true,
        image_data: rgb_image.into_raw(),
        image_filter: None,
        smask: None,
        clipping_bbox: None,
    });

    // ========== React版146-150行のアスペクト比計算そのまま ==========
    let img_aspect = width_px as f32 / height_px as f32;
    let box_aspect = box_width_pt / box_height_pt;

    let (draw_width_pt, draw_height_pt) = if img_aspect > box_aspect {
        // 横長画像: 幅にフィット
        (box_width_pt, box_width_pt / img_aspect)
    } else {
        // 縦長画像: 高さにフィット
        (box_height_pt * img_aspect, box_height_pt)
    };

    // ========== React版152-156行のセンタリング ==========
    let draw_x_pt = x_pt + (box_width_pt - draw_width_pt) / 2.0;
    let draw_y_pt = y_pt + (box_height_pt - draw_height_pt) / 2.0;

    // printpdfのスケール: 1px = 1pt として、目標サイズに合わせる
    let scale_x = draw_width_pt / width_px as f32;
    let scale_y = draw_height_pt / height_px as f32;

    image.add_to_layer(layer.clone(), ImageTransform {
        translate_x: Some(pt_to_mm(draw_x_pt)),
        translate_y: Some(pt_to_mm(draw_y_pt)),
        scale_x: Some(scale_x),
        scale_y: Some(scale_y),
        ..Default::default()
    });

    Ok(())
}

/// 矩形描画
fn draw_rect(layer: &PdfLayerReference, x_pt: f32, y_pt: f32, width_pt: f32, height_pt: f32) {
    let rect = Line {
        points: vec![
            (Point::new(pt_to_mm(x_pt), pt_to_mm(y_pt)), false),
            (Point::new(pt_to_mm(x_pt + width_pt), pt_to_mm(y_pt)), false),
            (Point::new(pt_to_mm(x_pt + width_pt), pt_to_mm(y_pt + height_pt)), false),
            (Point::new(pt_to_mm(x_pt), pt_to_mm(y_pt + height_pt)), false),
        ],
        is_closed: true,
    };
    layer.set_outline_color(Color::Rgb(Rgb::new(0.7, 0.7, 0.7, None)));
    layer.set_outline_thickness(0.5);
    layer.add_line(rect);
}

/// 情報欄テキスト描画（React版169-187行）
fn draw_info_fields(
    layer: &PdfLayerReference,
    result: &AnalysisResult,
    info_x_pt: f32,
    row_y_pt: f32,
    photo_height_pt: f32,
    fonts: &FontSet,
) {
    // React版169-174行のフィールド定義
    let fields = [
        ("工種", result.work_type.as_str()),
        ("種別", result.variety.as_str()),
        ("細別", result.detail.as_str()),
        ("測点", result.station.as_str()),
        ("備考", result.remarks.as_str()),
        ("測定値", result.measurements.as_str()),
    ];

    // React版180行: y = rowY + photoHeight - 15 - idx * 18
    for (idx, (label, value)) in fields.iter().enumerate() {
        let y_pt = row_y_pt + photo_height_pt - 15.0 - (idx as f32 * 18.0);
        if y_pt > row_y_pt + 5.0 {
            let label_text = process_text(label, fonts.is_japanese);
            let value_text = if value.is_empty() { "-" } else { value };
            let value_text = process_text(value_text, fonts.is_japanese);

            // ラベル
            layer.use_text(&format!("{}:", label_text), 8.0, pt_to_mm(info_x_pt + 5.0), pt_to_mm(y_pt), &fonts.regular);
            // 値（20文字で切り詰め）
            let truncated: String = value_text.chars().take(20).collect();
            layer.use_text(&truncated, 9.0, pt_to_mm(info_x_pt + 45.0), pt_to_mm(y_pt), &fonts.regular);
        }
    }

    // ファイル名（React版187行）
    layer.use_text(
        &result.file_name,
        7.0,
        pt_to_mm(info_x_pt + 5.0),
        pt_to_mm(row_y_pt + 5.0),
        &fonts.regular,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aspect_ratio_calculation() {
        // 横長画像（4:3）を縦長ボックス（1:2）にフィット
        let img_aspect = 4.0 / 3.0;
        let box_aspect = 100.0 / 200.0;

        let (w, h) = if img_aspect > box_aspect {
            (100.0, 100.0 / img_aspect)
        } else {
            (200.0 * img_aspect, 200.0)
        };

        assert!((w - 100.0).abs() < 0.01);
        assert!((h - 75.0).abs() < 0.01);
    }
}
