//! PDF生成モジュール
//!
//! ## 変更履歴
//! - 2026-01-18: テキスト自動調整を有効化（min_font_size: 8pt）
//! - 2026-01-18: printpdf 0.8アップグレード、フォントサブセット化対応

use crate::analyzer::AnalysisResult;
use crate::cli::PdfQuality;
use crate::error::{PhotoAiError, Result};
use super::layout::{self, mm_to_pt};
use printpdf::*;
use std::path::{Path, PathBuf};

use ::image as image_crate;
use image_crate::imageops::FilterType;

/// フォント情報
enum FontSet {
    Japanese(FontId),
    Builtin,
}

impl FontSet {
    fn is_japanese(&self) -> bool {
        matches!(self, FontSet::Japanese(_))
    }
}

/// 日本語フォントのパスを検索（明朝体優先）
fn find_japanese_font() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let windows_fonts = Path::new("C:\\Windows\\Fonts");
        // 明朝体を優先
        for font in ["YuMincho.ttc", "msmincho.ttc", "meiryo.ttc", "YuGothM.ttc", "msgothic.ttc"] {
            let path = windows_fonts.join(font);
            if path.exists() {
                return Some(path);
            }
        }
    }
    None
}

/// フォントをロード（printpdf 0.8 API）
fn load_fonts(doc: &mut PdfDocument) -> Result<FontSet> {
    if let Some(font_path) = find_japanese_font() {
        let font_data = std::fs::read(&font_path)?;
        let mut warnings = Vec::new();
        if let Some(parsed_font) = ParsedFont::from_bytes(&font_data, 0, &mut warnings) {
            let font_id = doc.add_font(&parsed_font);
            eprintln!("日本語フォント使用: {}", font_path.display());
            return Ok(FontSet::Japanese(font_id));
        }
    }

    // フォールバック: ビルトインフォント
    eprintln!("ビルトインフォント使用");
    Ok(FontSet::Builtin)
}

fn process_text(text: &str, is_japanese: bool) -> String {
    if is_japanese {
        text.to_string()
    } else {
        text.chars().map(|c| if c.is_ascii() { c } else { '?' }).collect()
    }
}

/// 統一フォントサイズ（12pt）
const UNIFIED_FONT_SIZE: f32 = 12.0;

/// 写真台帳PDFを生成（printpdf 0.8 API）
pub fn generate_pdf(
    results: &[AnalysisResult],
    output_path: &Path,
    photos_per_page: u8,
    title: &str,
    quality: PdfQuality,
) -> Result<()> {
    let photos_per_page = photos_per_page.max(2).min(3) as usize;

    // ========== React版と同一の定数（pt単位） ==========
    let a4_width_pt = mm_to_pt(layout::A4_WIDTH_MM);
    let a4_height_pt = mm_to_pt(layout::A4_HEIGHT_MM);
    let margin_pt = mm_to_pt(layout::MARGIN_MM);
    let header_height_pt: f32 = 40.0;
    let photo_info_gap_pt: f32 = 5.0;

    // 写真枠サイズ
    let photo_width_pt = mm_to_pt(layout::PHOTO_WIDTH_MM);
    let photo_height_pt = mm_to_pt(layout::PHOTO_HEIGHT_MM);
    let info_width_pt = mm_to_pt(layout::INFO_WIDTH_MM);
    let photo_row_height_pt = photo_height_pt + photo_info_gap_pt * 2.0;

    // printpdf 0.8: ドキュメント作成
    let mut doc = PdfDocument::new(title);
    let fonts = load_fonts(&mut doc)?;
    let total_pages = (results.len() + photos_per_page - 1) / photos_per_page;

    // 画像をドキュメントに追加してIDを取得
    let mut image_ids: Vec<Option<XObjectId>> = Vec::with_capacity(results.len());
    let mut image_sizes: Vec<(u32, u32)> = Vec::with_capacity(results.len());

    for result in results.iter() {
        if !result.file_path.is_empty() {
            match load_and_add_image(&mut doc, &result.file_path, quality) {
                Ok((id, w, h)) => {
                    image_ids.push(Some(id));
                    image_sizes.push((w, h));
                }
                Err(e) => {
                    eprintln!("警告: 写真読み込み失敗 ({}): {}", result.file_name, e);
                    image_ids.push(None);
                    image_sizes.push((0, 0));
                }
            }
        } else {
            image_ids.push(None);
            image_sizes.push((0, 0));
        }
    }

    // ページを生成
    let mut pages = Vec::new();

    for page_idx in 0..total_pages {
        let start_idx = page_idx * photos_per_page;
        let end_idx = (start_idx + photos_per_page).min(results.len());

        let mut ops = Vec::new();

        // ヘッダー描画
        add_header_ops(
            &mut ops,
            title,
            page_idx + 1,
            total_pages,
            &fonts,
            a4_width_pt,
            a4_height_pt,
            margin_pt,
        );

        // 各写真スロット
        for (slot, idx) in (start_idx..end_idx).enumerate() {
            let result = &results[idx];

            // Y座標計算（React版と同一）
            let row_y_pt = a4_height_pt - margin_pt - header_height_pt
                         - ((slot + 1) as f32 * photo_row_height_pt)
                         + photo_info_gap_pt;

            // 写真埋め込み
            if let Some(ref img_id) = image_ids[idx] {
                let (img_w, img_h) = image_sizes[idx];
                add_image_ops(
                    &mut ops,
                    img_id,
                    img_w,
                    img_h,
                    margin_pt,
                    row_y_pt,
                    photo_width_pt,
                    photo_height_pt,
                );
            }

            // 写真枠線
            add_rect_ops(&mut ops, margin_pt, row_y_pt, photo_width_pt, photo_height_pt);

            // 情報欄位置
            let info_x_pt = margin_pt + photo_width_pt + photo_info_gap_pt;

            // 情報欄枠線
            add_rect_ops(&mut ops, info_x_pt, row_y_pt, info_width_pt, photo_height_pt);

            // 情報欄テキスト
            add_info_field_ops(
                &mut ops,
                result,
                info_x_pt,
                row_y_pt,
                photo_height_pt,
                &fonts,
            );
        }

        let page = PdfPage::new(
            Mm(layout::A4_WIDTH_MM),
            Mm(layout::A4_HEIGHT_MM),
            ops,
        );
        pages.push(page);
    }

    // 出力先ディレクトリを確保
    if let Some(parent) = output_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    // printpdf 0.8: サブセット化で保存
    let save_options = PdfSaveOptions {
        subset_fonts: true,
        ..Default::default()
    };
    let mut warnings = Vec::new();
    let pdf_bytes = doc.with_pages(pages).save(&save_options, &mut warnings);

    // 警告があれば表示
    for warning in warnings {
        eprintln!("PDF警告: {:?}", warning);
    }

    std::fs::write(output_path, pdf_bytes)?;
    eprintln!("PDF保存完了: {}", output_path.display());

    Ok(())
}

/// 画像を読み込んでドキュメントに追加
fn load_and_add_image(
    doc: &mut PdfDocument,
    image_path: &str,
    quality: PdfQuality,
) -> Result<(XObjectId, u32, u32)> {
    let path = Path::new(image_path);
    if !path.exists() {
        return Err(PhotoAiError::FileNotFound(image_path.to_string()));
    }

    let dynamic_image = image_crate::open(path)
        .map_err(|e| PhotoAiError::PdfGeneration(format!("画像読み込みエラー: {}", e)))?;

    // 品質設定に基づいてリサイズ
    let resized = resize_image(dynamic_image, quality);
    let (width, height) = (resized.width(), resized.height());

    // JPEGエンコード（圧縮効率向上）
    let mut jpeg_bytes = Vec::new();
    let jpeg_quality = match quality {
        PdfQuality::High => 90,
        PdfQuality::Medium => 75,
        PdfQuality::Low => 60,
    };

    let rgb_image = resized.to_rgb8();
    let encoder = image_crate::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_bytes, jpeg_quality);
    rgb_image.write_with_encoder(encoder)
        .map_err(|e| PhotoAiError::PdfGeneration(format!("JPEG encode: {}", e)))?;

    eprintln!("  画像: {}x{}, JPEG {} bytes", width, height, jpeg_bytes.len());

    // printpdf 0.8: RawImageで追加
    let mut warnings = Vec::new();
    let raw_image = RawImage::decode_from_bytes(&jpeg_bytes, &mut warnings)
        .map_err(|e| PhotoAiError::PdfGeneration(format!("RawImage decode: {:?}", e)))?;

    let image_id = doc.add_image(&raw_image);

    Ok((image_id, width, height))
}

/// 画像をリサイズ（品質設定に基づく）
fn resize_image(
    img: image_crate::DynamicImage,
    quality: PdfQuality,
) -> image_crate::DynamicImage {
    let max_width = quality.max_width();
    let (orig_w, orig_h) = (img.width(), img.height());

    if orig_w <= max_width {
        return img;
    }

    let scale = max_width as f32 / orig_w as f32;
    let new_h = (orig_h as f32 * scale) as u32;

    img.resize(max_width, new_h, FilterType::Lanczos3)
}

/// テキスト描画オペレーション追加
fn add_text_ops(ops: &mut Vec<Op>, text: &str, x_pt: f32, y_pt: f32, size: f32, fonts: &FontSet) {
    ops.push(Op::StartTextSection);
    ops.push(Op::SetTextCursor { pos: Point { x: Pt(x_pt), y: Pt(y_pt) } });

    match fonts {
        FontSet::Japanese(font_id) => {
            ops.push(Op::SetFontSize { size: Pt(size), font: font_id.clone() });
            ops.push(Op::WriteText {
                items: vec![TextItem::Text(text.to_string())],
                font: font_id.clone(),
            });
        }
        FontSet::Builtin => {
            ops.push(Op::SetFontSizeBuiltinFont { size: Pt(size), font: BuiltinFont::Helvetica });
            ops.push(Op::WriteTextBuiltinFont {
                items: vec![TextItem::Text(text.to_string())],
                font: BuiltinFont::Helvetica,
            });
        }
    }

    ops.push(Op::EndTextSection);
}

/// テキスト描画オペレーション追加（Bold）
fn add_text_ops_bold(ops: &mut Vec<Op>, text: &str, x_pt: f32, y_pt: f32, size: f32, fonts: &FontSet) {
    ops.push(Op::StartTextSection);
    ops.push(Op::SetTextCursor { pos: Point { x: Pt(x_pt), y: Pt(y_pt) } });

    match fonts {
        FontSet::Japanese(font_id) => {
            // 日本語フォントはBold版がないので通常フォントを使用
            ops.push(Op::SetFontSize { size: Pt(size), font: font_id.clone() });
            ops.push(Op::WriteText {
                items: vec![TextItem::Text(text.to_string())],
                font: font_id.clone(),
            });
        }
        FontSet::Builtin => {
            ops.push(Op::SetFontSizeBuiltinFont { size: Pt(size), font: BuiltinFont::HelveticaBold });
            ops.push(Op::WriteTextBuiltinFont {
                items: vec![TextItem::Text(text.to_string())],
                font: BuiltinFont::HelveticaBold,
            });
        }
    }

    ops.push(Op::EndTextSection);
}

/// ヘッダー描画オペレーション追加
fn add_header_ops(
    ops: &mut Vec<Op>,
    title: &str,
    page_num: usize,
    total_pages: usize,
    fonts: &FontSet,
    a4_width_pt: f32,
    a4_height_pt: f32,
    margin_pt: f32,
) {
    let title_text = process_text(title, fonts.is_japanese());

    // タイトル
    ops.push(Op::SetFillColor { col: Color::Rgb(Rgb { r: 0.0, g: 0.0, b: 0.0, icc_profile: None }) });
    add_text_ops_bold(
        ops,
        &title_text,
        margin_pt,
        a4_height_pt - margin_pt - 20.0,
        UNIFIED_FONT_SIZE,
        fonts,
    );

    // ページ番号
    add_text_ops(
        ops,
        &format!("Page {} / {}", page_num, total_pages),
        a4_width_pt - margin_pt - 80.0,
        a4_height_pt - margin_pt - 20.0,
        UNIFIED_FONT_SIZE,
        fonts,
    );
}

/// 画像描画オペレーション追加
fn add_image_ops(
    ops: &mut Vec<Op>,
    image_id: &XObjectId,
    img_width: u32,
    img_height: u32,
    x_pt: f32,
    y_pt: f32,
    box_width_pt: f32,
    box_height_pt: f32,
) {
    // アスペクト比を維持して枠いっぱいにフィット
    let img_aspect = img_width as f32 / img_height as f32;
    let box_aspect = box_width_pt / box_height_pt;

    let (draw_width_pt, draw_height_pt) = if img_aspect > box_aspect {
        (box_width_pt, box_width_pt / img_aspect)
    } else {
        (box_height_pt * img_aspect, box_height_pt)
    };

    // センタリング
    let draw_x_pt = x_pt + (box_width_pt - draw_width_pt) / 2.0;
    let draw_y_pt = y_pt + (box_height_pt - draw_height_pt) / 2.0;

    // printpdf 0.8: DPIを計算して画像サイズを制御
    // 公式: img_width_px * (72 / dpi) = draw_width_pt
    // よって: dpi = img_width_px * 72 / draw_width_pt
    let dpi = img_width as f32 * 72.0 / draw_width_pt;

    ops.push(Op::UseXobject {
        id: image_id.clone(),
        transform: XObjectTransform {
            translate_x: Some(Pt(draw_x_pt)),
            translate_y: Some(Pt(draw_y_pt)),
            dpi: Some(dpi),
            ..Default::default()
        },
    });
}

/// 矩形描画オペレーション追加
fn add_rect_ops(ops: &mut Vec<Op>, x_pt: f32, y_pt: f32, width_pt: f32, height_pt: f32) {
    ops.push(Op::SetOutlineColor { col: Color::Rgb(Rgb { r: 0.7, g: 0.7, b: 0.7, icc_profile: None }) });
    ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });

    let points = vec![
        LinePoint { p: Point { x: Pt(x_pt), y: Pt(y_pt) }, bezier: false },
        LinePoint { p: Point { x: Pt(x_pt + width_pt), y: Pt(y_pt) }, bezier: false },
        LinePoint { p: Point { x: Pt(x_pt + width_pt), y: Pt(y_pt + height_pt) }, bezier: false },
        LinePoint { p: Point { x: Pt(x_pt), y: Pt(y_pt + height_pt) }, bezier: false },
    ];

    ops.push(Op::DrawPolygon {
        polygon: Polygon {
            rings: vec![PolygonRing { points }],
            mode: PaintMode::Stroke,
            winding_order: WindingOrder::NonZero,
        },
    });
}

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

/// テキスト自動調整設定
struct TextFitConfig {
    max_width_chars: usize,
    base_font_size: f32,
    min_font_size: f32,
    max_lines: usize,
}

impl Default for TextFitConfig {
    fn default() -> Self {
        Self {
            max_width_chars: 15,
            base_font_size: UNIFIED_FONT_SIZE,
            min_font_size: 8.0, // 自動縮小の下限
            max_lines: 2,
        }
    }
}

/// テキスト描画オペレーション追加（自動調整）
fn add_fitted_text_ops(
    ops: &mut Vec<Op>,
    text: &str,
    x_pt: f32,
    y_pt: f32,
    fonts: &FontSet,
    config: &TextFitConfig,
) {
    if text.is_empty() {
        return;
    }

    let char_count = text.chars().count();

    let font_size = if char_count <= config.max_width_chars {
        config.base_font_size
    } else if char_count <= config.max_width_chars * 2 {
        let ratio = config.max_width_chars as f32 / char_count as f32;
        (config.base_font_size * ratio).max(config.min_font_size)
    } else {
        config.min_font_size
    };

    let chars_per_line = ((config.max_width_chars as f32 * config.base_font_size / font_size) as usize).max(10);
    let total_max_chars = chars_per_line * config.max_lines;

    if char_count <= chars_per_line {
        add_text_ops(ops, text, x_pt, y_pt, font_size, fonts);
    } else if char_count <= total_max_chars {
        let (line1, line2) = text.split_at(text.char_indices().nth(chars_per_line).map(|(i, _)| i).unwrap_or(text.len()));
        add_text_ops(ops, line1, x_pt, y_pt, font_size, fonts);
        add_text_ops(ops, line2, x_pt, y_pt - 10.0, font_size, fonts);
    } else {
        let max_chars = total_max_chars - 1;
        let truncated: String = text.chars().take(max_chars).chain(std::iter::once('…')).collect();
        let (line1, line2) = truncated.split_at(truncated.char_indices().nth(chars_per_line).map(|(i, _)| i).unwrap_or(truncated.len()));
        add_text_ops(ops, line1, x_pt, y_pt, font_size, fonts);
        add_text_ops(ops, line2, x_pt, y_pt - 10.0, font_size, fonts);
    }
}

/// 情報欄テキスト描画オペレーション追加
fn add_info_field_ops(
    ops: &mut Vec<Op>,
    result: &AnalysisResult,
    info_x_pt: f32,
    row_y_pt: f32,
    photo_height_pt: f32,
    fonts: &FontSet,
) {
    let mut y_offset = 0u8;
    let text_config = TextFitConfig::default();

    ops.push(Op::SetFillColor { col: Color::Rgb(Rgb { r: 0.0, g: 0.0, b: 0.0, icc_profile: None }) });

    for field in layout::LAYOUT_FIELDS.iter() {
        let y_pt = row_y_pt + photo_height_pt - 15.0 - (y_offset as f32 * 18.0);

        if y_pt > row_y_pt + 5.0 {
            let label_text = process_text(field.label, fonts.is_japanese());
            let value = get_field_value(result, field.key);
            let value_text = if value.is_empty() { "-" } else { value };
            let value_text = process_text(value_text, fonts.is_japanese());

            // ラベル
            add_text_ops(ops, &format!("{}:", label_text), info_x_pt + 5.0, y_pt, UNIFIED_FONT_SIZE, fonts);

            // 値（自動調整）
            add_fitted_text_ops(ops, &value_text, info_x_pt + 45.0, y_pt, fonts, &text_config);
        }

        y_offset += field.row_span;
    }

    // ファイル名
    add_text_ops(ops, &result.file_name, info_x_pt + 5.0, row_y_pt + 5.0, UNIFIED_FONT_SIZE, fonts);
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_aspect_ratio_calculation() {
        let img_aspect: f64 = 4.0 / 3.0;
        let box_aspect: f64 = 100.0 / 200.0;

        let (w, h): (f64, f64) = if img_aspect > box_aspect {
            (100.0, 100.0 / img_aspect)
        } else {
            (200.0 * img_aspect, 200.0)
        };

        assert!((w - 100.0).abs() < 0.01);
        assert!((h - 75.0).abs() < 0.01);
    }
}
