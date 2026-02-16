//! PDF生成モジュール
//!
//! ## 変更履歴
//! - 2026-01-18: テキスト自動調整を有効化（min_font_size: 8pt）
//! - 2026-01-18: printpdf 0.8アップグレード、フォントサブセット化対応

use crate::analyzer::AnalysisResult;
use crate::cli::PdfQuality;
use crate::error::{PhotoAiError, Result};
use photo_ai_common::export::pdf as pdf_common;
use photo_ai_common::PdfLayout;
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
    // 日本語フォント候補（明朝体優先）
    let font_names = [
        // 明朝体
        "NotoSerifCJKjp-Regular.otf",
        "NotoSerifCJK-Regular.ttc",
        "IPAMincho.ttf",
        "ipaexm.ttf",
        // ゴシック体
        "NotoSansCJKjp-Regular.otf",
        "NotoSansCJK-Regular.ttc",
        "IPAGothic.ttf",
        "ipaexg.ttf",
    ];

    #[cfg(windows)]
    {
        let windows_fonts = Path::new("C:\\Windows\\Fonts");
        // Windows特有のフォント（明朝体優先）
        for font in ["YuMincho.ttc", "msmincho.ttc", "meiryo.ttc", "YuGothM.ttc", "msgothic.ttc"] {
            let path = windows_fonts.join(font);
            if path.exists() {
                return Some(path);
            }
        }
        // Noto/IPAフォント
        for font in &font_names {
            let path = windows_fonts.join(font);
            if path.exists() {
                return Some(path);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let font_dirs = [
            Path::new("/System/Library/Fonts"),
            Path::new("/Library/Fonts"),
            Path::new(&format!("{}/Library/Fonts", std::env::var("HOME").unwrap_or_default())),
        ];
        // macOS特有のフォント
        for dir in &font_dirs {
            for font in ["ヒラギノ明朝 ProN.ttc", "Hiragino Mincho ProN.ttc", "ヒラギノ角ゴシック.ttc"] {
                let path = dir.join(font);
                if path.exists() {
                    return Some(path);
                }
            }
            for font in &font_names {
                let path = dir.join(font);
                if path.exists() {
                    return Some(path);
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let home_fonts = format!("{}/.local/share/fonts", std::env::var("HOME").unwrap_or_default());
        let font_dirs = [
            Path::new("/usr/share/fonts/opentype/noto"),
            Path::new("/usr/share/fonts/truetype/noto"),
            Path::new("/usr/share/fonts/opentype/ipafont-mincho"),
            Path::new("/usr/share/fonts/truetype/ipafont"),
            Path::new("/usr/share/fonts/OTF"),
            Path::new("/usr/share/fonts/TTF"),
            Path::new(&home_fonts),
        ];
        for dir in &font_dirs {
            for font in &font_names {
                let path = dir.join(font);
                if path.exists() {
                    return Some(path);
                }
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
    hide_measurements: bool,
) -> Result<()> {
    let photos_per_page = photos_per_page.clamp(2, 3);
    let layout = PdfLayout::for_photos_per_page(photos_per_page);
    let layout_core = pdf_common::PdfMetrics::from_layout(&layout);

    // printpdf 0.8: ドキュメント作成
    let mut doc = PdfDocument::new(title);
    let fonts = load_fonts(&mut doc)?;
    let total_pages = results.len().div_ceil(layout_core.photos_per_page);

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
        let start_idx = page_idx * layout_core.photos_per_page;
        let end_idx = (start_idx + layout_core.photos_per_page).min(results.len());

        let mut ops = Vec::new();

        // ページ番号のみ描画（ヘッダータイトルは写真エリアと干渉するため省略）
        add_page_number_ops(&mut ops, page_idx + 1, &fonts, &layout_core);

        // 各写真スロット
        for (slot, idx) in (start_idx..end_idx).enumerate() {
            let result = &results[idx];

            // 空エントリー（スペーサー）: fileNameが空ならスロットを空白にする
            if result.file_name.is_empty() {
                continue;
            }

            // Y座標計算（React版と同一）
            let row_y_pt = layout_core.row_y_pt(slot);

            // 写真埋め込み
            if let Some(ref img_id) = image_ids[idx] {
                let (img_w, img_h) = image_sizes[idx];
                add_image_ops(
                    &mut ops,
                    img_id,
                    img_w,
                    img_h,
                    layout_core.margin_pt,
                    row_y_pt,
                    layout_core.photo_width_pt,
                    layout_core.photo_height_pt,
                );
            }

            // 写真枠線
            add_rect_ops(
                &mut ops,
                layout_core.margin_pt,
                row_y_pt,
                layout_core.photo_width_pt,
                layout_core.photo_height_pt,
            );

            // 情報欄位置
            let info_x_pt = layout_core.info_x_pt();

            // 情報欄枠線
            add_rect_ops(
                &mut ops,
                info_x_pt,
                row_y_pt,
                layout_core.info_width_pt,
                layout_core.photo_height_pt,
            );

            // 情報欄テキスト
            add_info_field_ops(
                &mut ops,
                result,
                info_x_pt,
                row_y_pt,
                layout_core.photo_height_pt,
                &fonts,
                hide_measurements,
            );
        }

        let page = PdfPage::new(
            Mm(layout.page_width_mm),
            Mm(layout.page_height_mm),
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

/// ページ番号のみ描画
fn add_page_number_ops(
    ops: &mut Vec<Op>,
    page_num: usize,
    fonts: &FontSet,
    layout: &pdf_common::PdfMetrics,
) {
    ops.push(Op::SetFillColor { col: Color::Rgb(Rgb { r: 0.0, g: 0.0, b: 0.0, icc_profile: None }) });
    add_text_ops(
        ops,
        &format!("Page {}", page_num),
        layout.page_width_pt - layout.margin_pt - 50.0,
        layout.page_height_pt - layout.margin_pt + 2.0,
        9.0,
        fonts,
    );
}

/// 画像描画オペレーション追加
#[allow(clippy::too_many_arguments)]
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

/// テキスト自動調整設定
struct TextFitConfig {
    /// 半角換算での最大幅（全角=2, 半角=1）
    max_half_width: usize,
    base_font_size: f32,
    min_font_size: f32,
    max_lines: usize,
}

impl Default for TextFitConfig {
    fn default() -> Self {
        Self {
            max_half_width: 22,
            base_font_size: UNIFIED_FONT_SIZE,
            min_font_size: 10.0,
            max_lines: 2,
        }
    }
}

/// 半角換算の文字幅を計算（全角=2, 半角=1）
fn half_width_count(text: &str) -> usize {
    text.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum()
}

/// カタカナ文字判定（U+30A0〜U+30FF）
fn is_katakana(c: char) -> bool {
    ('\u{30A0}'..='\u{30FF}').contains(&c)
}

/// 半角換算幅で文字列を分割（スペース区切り・文字種境界を優先）
fn split_at_half_width(text: &str, max_hw: usize) -> (&str, &str) {
    let mut hw = 0;
    // 最後の「単語の開始位置」を記録（スペース/区切り/文字種境界）
    let mut last_word_start = None;
    let mut prev_was_sep = false;
    let mut prev_char: Option<char> = None;
    for (i, c) in text.char_indices() {
        let is_sep = c == ' ' || c == '/' || c == '\u{3000}';
        if !prev_was_sep && is_sep {
            // 非区切り→区切り: ここが折り返し候補
            last_word_start = Some(i);
        }
        // 文字種境界で折り返し候補を記録
        if let Some(pc) = prev_char {
            if !is_sep && !prev_was_sep && !c.is_ascii() {
                // カタカナ↔非カタカナの境界: "タックコート乳剤" → "ト"と"乳"の間
                // 「工」の直後: "路面切削工出来形測定" → "工"と"出"の間
                if is_katakana(pc) != is_katakana(c) || pc == '工' {
                    last_word_start = Some(i);
                }
            }
        }
        prev_was_sep = is_sep;
        prev_char = Some(c);
        hw += if c.is_ascii() { 1 } else { 2 };
        if hw > max_hw {
            if let Some(sp) = last_word_start {
                let line2 = text[sp..].trim_start_matches(|c: char| c == ' ' || c == '/' || c == '\u{3000}');
                return (text[..sp].trim_end(), line2);
            }
            return (&text[..i], &text[i..]);
        }
    }
    (text, "")
}

/// テキストの描画情報（行分割結果・フォントサイズ・行間）
struct TextLayout<'a> {
    lines: Vec<&'a str>,
    font_size: f32,
    line_spacing: f32,
}

impl TextFitConfig {
    /// テキストを行分割し、適切なフォントサイズを決定する
    fn layout_for_height<'a>(&self, text: &'a str, _field_height: f32) -> TextLayout<'a> {
        // 明示的改行（フォントサイズは幅で決定、高さでは縮小しない）
        if text.contains('\n') {
            let lines: Vec<&str> = text.split('\n').map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
            let max_hw = lines.iter().map(|l| half_width_count(l)).max().unwrap_or(0);
            let font_size = if max_hw <= self.max_half_width {
                self.base_font_size
            } else {
                self.min_font_size
            };
            return TextLayout { lines, font_size, line_spacing: font_size + 1.0 };
        }

        let hw = half_width_count(text);
        if hw <= self.max_half_width {
            return TextLayout { lines: vec![text], font_size: self.base_font_size, line_spacing: self.base_font_size + 1.0 };
        }

        let hw_at_smaller = (self.max_half_width as f32 * self.base_font_size / self.min_font_size) as usize;
        if hw <= hw_at_smaller {
            return TextLayout { lines: vec![text], font_size: self.min_font_size, line_spacing: self.min_font_size + 1.0 };
        }

        // 10ptで自動改行
        let (line1, line2) = split_at_half_width(text, hw_at_smaller);
        let mut lines = vec![line1];
        if !line2.is_empty() {
            lines.push(line2);
        }
        TextLayout { lines, font_size: self.min_font_size, line_spacing: self.min_font_size + 1.0 }
    }

    /// フィールド中央Y座標から、テキストブロックの1行目ベースラインY座標を計算
    fn centered_first_line_y(&self, text: &str, field_center_y: f32, field_height: f32) -> f32 {
        let layout = self.layout_for_height(text, field_height);
        let n = layout.lines.len() as f32;
        let baselines_span = layout.line_spacing * (n - 1.0);
        // ブロック中央のベースライン = field_center - font*0.3（ラベルと同一）
        // 1行目 = 中央 + baselines_span/2
        field_center_y + baselines_span / 2.0 - layout.font_size * 0.3
    }
}

/// テキスト描画オペレーション追加（センタリング済みのy_ptから描画）
fn add_fitted_text_ops(
    ops: &mut Vec<Op>,
    text: &str,
    x_pt: f32,
    y_pt: f32,
    field_height: f32,
    fonts: &FontSet,
    config: &TextFitConfig,
) {
    if text.is_empty() {
        return;
    }

    let layout = config.layout_for_height(text, field_height);
    for (i, line) in layout.lines.iter().enumerate() {
        add_text_ops(ops, line, x_pt, y_pt - layout.line_spacing * i as f32, layout.font_size, fonts);
    }
}

/// 測定値フィールド描画（実測値行を赤色で描画）
fn add_measurements_text_ops(
    ops: &mut Vec<Op>,
    text: &str,
    x_pt: f32,
    y_pt: f32,
    field_height: f32,
    fonts: &FontSet,
    config: &TextFitConfig,
) {
    if text.is_empty() {
        return;
    }

    let layout = config.layout_for_height(text, field_height);
    let black = Color::Rgb(Rgb { r: 0.0, g: 0.0, b: 0.0, icc_profile: None });
    let red = Color::Rgb(Rgb { r: 0.8, g: 0.0, b: 0.0, icc_profile: None });

    for (i, line) in layout.lines.iter().enumerate() {
        let line_y = y_pt - layout.line_spacing * i as f32;
        let has_actual = line.contains("実施") || line.contains("実測");
        let has_design = line.contains("設計");

        if has_actual && has_design {
            // 設計と実測が同居する行（例: "幅員W1 設計: 4.20 実測: 4.20"）
            // 実測部分だけ赤にする
            let keyword = if line.contains("実測") { "実測" } else { "実施" };
            if let Some(pos) = line.find(keyword) {
                let before = &line[..pos];
                let after = &line[pos..];
                ops.push(Op::SetFillColor { col: black.clone() });
                add_text_ops(ops, before, x_pt, line_y, layout.font_size, fonts);
                let before_width = half_width_count(before) as f32 * layout.font_size / 2.0;
                ops.push(Op::SetFillColor { col: red.clone() });
                add_text_ops(ops, after, x_pt + before_width, line_y, layout.font_size, fonts);
            }
        } else if has_actual {
            ops.push(Op::SetFillColor { col: red.clone() });
            add_text_ops(ops, line, x_pt, line_y, layout.font_size, fonts);
        } else {
            ops.push(Op::SetFillColor { col: black.clone() });
            add_text_ops(ops, line, x_pt, line_y, layout.font_size, fonts);
        }
    }
    // 色を黒に戻す
    ops.push(Op::SetFillColor { col: black });
}

/// 水平線描画オペレーション追加
fn add_horizontal_line_ops(ops: &mut Vec<Op>, x1_pt: f32, x2_pt: f32, y_pt: f32) {
    ops.push(Op::SetOutlineColor { col: Color::Rgb(Rgb { r: 0.7, g: 0.7, b: 0.7, icc_profile: None }) });
    ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });

    let points = vec![
        LinePoint { p: Point { x: Pt(x1_pt), y: Pt(y_pt) }, bezier: false },
        LinePoint { p: Point { x: Pt(x2_pt), y: Pt(y_pt) }, bezier: false },
    ];

    ops.push(Op::DrawPolygon {
        polygon: Polygon {
            rings: vec![PolygonRing { points }],
            mode: PaintMode::Stroke,
            winding_order: WindingOrder::NonZero,
        },
    });
}

/// 情報欄テキスト描画オペレーション追加（参照PDF準拠レイアウト）
fn add_info_field_ops(
    ops: &mut Vec<Op>,
    result: &AnalysisResult,
    info_x_pt: f32,
    row_y_pt: f32,
    photo_height_pt: f32,
    fonts: &FontSet,
    hide_measurements: bool,
) {
    let fields = pdf_common::build_pdf_info_fields_opt(result, hide_measurements);
    let label_width = 35.0;

    // row_spanの合計で1単位あたりの高さを計算
    let total_spans: f32 = fields.iter().map(|f| f.row_span as f32).sum();
    let unit_height = photo_height_pt / (total_spans + 0.5);

    ops.push(Op::SetFillColor { col: Color::Rgb(Rgb { r: 0.0, g: 0.0, b: 0.0, icc_profile: None }) });

    let text_config = TextFitConfig::default();

    // 各フィールドのtop位置を累積計算
    let mut current_top = row_y_pt + photo_height_pt;

    // 測定値フィールド用: ラベル右側に通常フォントサイズで描画
    let wide_text_config = TextFitConfig {
        max_half_width: 20,
        ..TextFitConfig::default()
    };

    // 備考フィールド用: 長い備考（タックコート乳剤散布状況等）を同じフォントサイズで改行
    let remarks_text_config = TextFitConfig {
        max_half_width: 20,
        min_font_size: UNIFIED_FONT_SIZE, // 縮小せず改行のみ
        ..TextFitConfig::default()
    };

    for (i, field) in fields.iter().enumerate() {
        let field_height = unit_height * field.row_span as f32;
        let field_bottom = current_top - field_height;

        if field_bottom >= row_y_pt {
            let label_text = process_text(&field.label, fonts.is_japanese());
            let value_text = process_text(&field.value, fonts.is_japanese());

            let field_center = field_bottom + field_height / 2.0;

            // 測定値フィールド: ラベル表示 + 実測値は赤で描画
            let is_measurements = field.label == "測定値";
            if is_measurements && value_text != "-" {
                let label_y = field_center - UNIFIED_FONT_SIZE * 0.3;
                let text_y = wide_text_config.centered_first_line_y(&value_text, field_center, field_height);
                add_text_ops(ops, &label_text, info_x_pt + 5.0, label_y, UNIFIED_FONT_SIZE, fonts);
                add_measurements_text_ops(ops, &value_text, info_x_pt + label_width + 10.0, text_y, field_height, fonts, &wide_text_config);
            } else {
                let is_remarks = field.label == "備考";
                let cfg = if is_remarks { &remarks_text_config } else { &text_config };
                let label_y = field_center - UNIFIED_FONT_SIZE * 0.3;
                let text_y = cfg.centered_first_line_y(&value_text, field_center, field_height);
                add_text_ops(ops, &label_text, info_x_pt + 5.0, label_y, UNIFIED_FONT_SIZE, fonts);
                add_fitted_text_ops(ops, &value_text, info_x_pt + label_width + 10.0, text_y, field_height, fonts, cfg);
            }

            // 行の下に水平線（最後の行以外）
            if i < fields.len() - 1 {
                add_horizontal_line_ops(ops, info_x_pt, info_x_pt + 180.0, field_bottom);
            }
        }

        current_top = field_bottom;
    }
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
