//! 着手前竣工写真帳PDF生成モジュール
//!
//! ペアリング済みフォルダ（P01_測点名/before.JPG + after.jpg）から
//! A4横の写真帳PDFを生成する。

use crate::error::{PhotoAiError, Result};
use printpdf::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use ::image as image_crate;
use image_crate::imageops::FilterType;

// ---- レイアウト定数 ----
const PAGE_W: f32 = 842.0;
const PAGE_H: f32 = 595.35;
const MARGIN_X: f32 = 19.2; // 左右マージン（PIL: 40px）
const MARGIN_TOP: f32 = 28.35;   // 1cm
const MARGIN_BOTTOM: f32 = 28.35; // 1cm

// ---- フォントサイズ（pt, PIL: px * 0.48）----
const TITLE_SIZE: f32 = 32.0;
const PROJECT_NAME_SIZE: f32 = 24.0;
const CAPTION_FONT_SIZE: f32 = 12.0;

// ---- キャプション固定Y座標（1行: 罫線上下 + テキスト）----
const RULE_BOTTOM_Y: f32 = MARGIN_BOTTOM + 4.0;
const CAPTION_Y: f32 = MARGIN_BOTTOM + 10.0;
const RULE_TOP_Y: f32 = MARGIN_BOTTOM + 20.0;

// ---- 写真領域 ----
const PHOTO_X: f32 = MARGIN_X;
const PHOTO_Y: f32 = RULE_TOP_Y + 8.0;
const PHOTO_W: f32 = PAGE_W - MARGIN_X * 2.0;
const PHOTO_H: f32 = PAGE_H - MARGIN_TOP - PHOTO_Y; // 595.35 - 28.35 - 72 = 495pt

// ---- キャプション ----
const RULE_W: f32 = 264.0;

// ---- 画像品質 ----
const MAX_IMAGE_WIDTH: u32 = 1400;

/// ペアリング結果の1エントリ
#[derive(Serialize, Deserialize)]
pub struct PairEntry {
    pub station_name: String,
    pub before_path: PathBuf,
    pub after_path: PathBuf,
}

/// フォント選択
enum FontChoice {
    External(FontId),
    Builtin,
}

impl FontChoice {
    fn push_text_ops(&self, ops: &mut Vec<Op>, text: &str, x: f32, y: f32, size: f32) {
        ops.push(Op::StartTextSection);
        ops.push(Op::SetTextCursor { pos: Point { x: Pt(x), y: Pt(y) } });
        match self {
            FontChoice::External(id) => {
                ops.push(Op::SetFontSize { size: Pt(size), font: id.clone() });
                ops.push(Op::WriteText {
                    items: vec![TextItem::Text(text.to_string())],
                    font: id.clone(),
                });
            }
            FontChoice::Builtin => {
                ops.push(Op::SetFontSizeBuiltinFont { size: Pt(size), font: BuiltinFont::Helvetica });
                ops.push(Op::WriteTextBuiltinFont {
                    items: vec![TextItem::Text(text.to_string())],
                    font: BuiltinFont::Helvetica,
                });
            }
        }
        ops.push(Op::EndTextSection);
    }
}

/// タイトル用・キャプション用のフォントセット
struct PairFonts {
    title: FontChoice,
    caption: FontChoice,
}

/// 半角換算でテキスト幅を推定（全角=1em, 半角=0.5em）
fn estimate_text_width(text: &str, font_size: f32) -> f32 {
    let units: f32 = text.chars().map(|c| if c.is_ascii() { 0.5 } else { 1.0 }).sum();
    units * font_size
}

/// テキストをページ中央揃えにするX座標を計算
fn center_x(text: &str, font_size: f32) -> f32 {
    let w = estimate_text_width(text, font_size);
    (PAGE_W - w) / 2.0
}

/// 罫線の中央X開始位置
fn rule_start_x() -> f32 {
    (PAGE_W - RULE_W) / 2.0
}

/// 游明朝フォントをロード（Demibold + Light、なければフォールバック）
fn load_fonts(doc: &mut PdfDocument) -> Result<PairFonts> {
    let mut title_font = None;
    let mut caption_font = None;

    #[cfg(windows)]
    {
        let fonts_dir = Path::new("C:\\Windows\\Fonts");

        // 游明朝Demibold（タイトル用）
        let db_path = fonts_dir.join("yumindb.ttf");
        if db_path.exists() {
            if let Ok(data) = std::fs::read(&db_path) {
                let mut warnings = Vec::new();
                if let Some(parsed) = ParsedFont::from_bytes(&data, 0, &mut warnings) {
                    title_font = Some(doc.add_font(&parsed));
                    eprintln!("タイトルフォント: 游明朝Demibold");
                }
            }
        }

        // 游明朝Light（キャプション用）
        let lt_path = fonts_dir.join("yuminl.ttf");
        if lt_path.exists() {
            if let Ok(data) = std::fs::read(&lt_path) {
                let mut warnings = Vec::new();
                if let Some(parsed) = ParsedFont::from_bytes(&data, 0, &mut warnings) {
                    caption_font = Some(doc.add_font(&parsed));
                    eprintln!("キャプションフォント: 游明朝Light");
                }
            }
        }
    }

    // フォールバック: 既存の日本語フォント検索
    let fallback = if title_font.is_none() || caption_font.is_none() {
        super::pdf::find_japanese_font().and_then(|path| {
            let data = std::fs::read(&path).ok()?;
            let mut warnings = Vec::new();
            let parsed = ParsedFont::from_bytes(&data, 0, &mut warnings)?;
            let id = doc.add_font(&parsed);
            eprintln!("フォールバックフォント: {}", path.display());
            Some(id)
        })
    } else {
        None
    };

    let title = match title_font {
        Some(id) => FontChoice::External(id),
        None => match fallback.clone() {
            Some(id) => FontChoice::External(id),
            None => FontChoice::Builtin,
        },
    };

    let caption = match caption_font {
        Some(id) => FontChoice::External(id),
        None => match fallback {
            Some(id) => FontChoice::External(id),
            None => {
                eprintln!("警告: 日本語フォントが見つかりません。テキストが正しく表示されない可能性があります");
                FontChoice::Builtin
            }
        },
    };

    Ok(PairFonts { title, caption })
}

/// 画像を読み込んでドキュメントに追加（高品質固定）
/// 戻り値: (XObjectId, width, height, jpeg_bytes)
fn load_and_add_image(doc: &mut PdfDocument, path: &Path) -> Result<(XObjectId, u32, u32, Vec<u8>)> {
    if !path.exists() {
        return Err(PhotoAiError::FileNotFound(path.display().to_string()));
    }

    let file_bytes = std::fs::read(path)
        .map_err(|e| PhotoAiError::PdfGeneration(format!("ファイル読み込みエラー: {}", e)))?;

    let img = image_crate::load_from_memory(&file_bytes)
        .map_err(|e| PhotoAiError::PdfGeneration(format!("画像デコードエラー: {}", e)))?;

    let (orig_w, orig_h) = (img.width(), img.height());

    let jpeg_bytes = if orig_w > MAX_IMAGE_WIDTH {
        // リサイズが必要 → 再エンコード
        let scale = MAX_IMAGE_WIDTH as f32 / orig_w as f32;
        let new_h = (orig_h as f32 * scale) as u32;
        let resized = img.resize(MAX_IMAGE_WIDTH, new_h, FilterType::Lanczos3);
        let rgb = resized.to_rgb8();
        let mut buf = Vec::new();
        let encoder = image_crate::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 95);
        rgb.write_with_encoder(encoder)
            .map_err(|e| PhotoAiError::PdfGeneration(format!("JPEG encode: {}", e)))?;
        let (w, h) = (resized.width(), resized.height());
        eprintln!("  画像: {}x{} (リサイズ), {} bytes ({})", w, h, buf.len(), path.file_name().unwrap_or_default().to_string_lossy());

        let mut warnings = Vec::new();
        let raw = RawImage::decode_from_bytes(&buf, &mut warnings)
            .map_err(|e| PhotoAiError::PdfGeneration(format!("RawImage: {:?}", e)))?;
        let id = doc.add_image(&raw);
        return Ok((id, w, h, buf));
    } else {
        // リサイズ不要 → 元JPEGバイトをそのまま使用
        file_bytes
    };

    eprintln!("  画像: {}x{}, {} bytes ({})", orig_w, orig_h, jpeg_bytes.len(), path.file_name().unwrap_or_default().to_string_lossy());

    let mut warnings = Vec::new();
    let raw = RawImage::decode_from_bytes(&jpeg_bytes, &mut warnings)
        .map_err(|e| PhotoAiError::PdfGeneration(format!("RawImage: {:?}", e)))?;

    let id = doc.add_image(&raw);
    Ok((id, orig_w, orig_h, jpeg_bytes))
}

/// 写真をフィットさせて描画
fn add_photo_ops(ops: &mut Vec<Op>, id: &XObjectId, img_w: u32, img_h: u32) {
    let img_aspect = img_w as f32 / img_h as f32;
    let box_aspect = PHOTO_W / PHOTO_H;

    let (draw_w, draw_h) = if img_aspect > box_aspect {
        (PHOTO_W, PHOTO_W / img_aspect)
    } else {
        (PHOTO_H * img_aspect, PHOTO_H)
    };

    let draw_x = PHOTO_X + (PHOTO_W - draw_w) / 2.0;
    let draw_y = PHOTO_Y + (PHOTO_H - draw_h) / 2.0;

    let dpi = img_w as f32 * 72.0 / draw_w;

    ops.push(Op::UseXobject {
        id: id.clone(),
        transform: XObjectTransform {
            translate_x: Some(Pt(draw_x)),
            translate_y: Some(Pt(draw_y)),
            dpi: Some(dpi),
            ..Default::default()
        },
    });
}

/// 罫線（水平線）を描画
fn add_rule_ops(ops: &mut Vec<Op>, y: f32) {
    let x1 = rule_start_x();
    let x2 = x1 + RULE_W;

    ops.push(Op::SetOutlineColor { col: Color::Rgb(Rgb { r: 0.3, g: 0.3, b: 0.3, icc_profile: None }) });
    ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });

    let points = vec![
        LinePoint { p: Point { x: Pt(x1), y: Pt(y) }, bezier: false },
        LinePoint { p: Point { x: Pt(x2), y: Pt(y) }, bezier: false },
    ];

    ops.push(Op::DrawPolygon {
        polygon: Polygon {
            rings: vec![PolygonRing { points }],
            mode: PaintMode::Stroke,
            winding_order: WindingOrder::NonZero,
        },
    });
}

/// 表紙ページを生成
fn build_cover_page(fonts: &PairFonts, project_name: &str) -> PdfPage {
    let mut ops = Vec::new();
    ops.push(Op::SetFillColor { col: Color::Rgb(Rgb { r: 0.0, g: 0.0, b: 0.0, icc_profile: None }) });

    let title = "着手前-竣工写真";
    let title_x = center_x(title, TITLE_SIZE);
    let title_y = PAGE_H / 2.0 + 30.0;
    fonts.title.push_text_ops(&mut ops, title, title_x, title_y, TITLE_SIZE);

    let name_x = center_x(project_name, PROJECT_NAME_SIZE);
    let name_y = PAGE_H / 2.0 - 30.0;
    fonts.title.push_text_ops(&mut ops, project_name, name_x, name_y, PROJECT_NAME_SIZE);

    PdfPage::new(Mm(297.0), Mm(210.0), ops)
}

/// 写真ページを生成（着手前 or 竣工）
fn build_photo_page(
    fonts: &PairFonts,
    image_id: &XObjectId,
    img_w: u32,
    img_h: u32,
    caption: &str,
) -> PdfPage {
    let mut ops = Vec::new();
    ops.push(Op::SetFillColor { col: Color::Rgb(Rgb { r: 0.0, g: 0.0, b: 0.0, icc_profile: None }) });

    // 写真描画
    add_photo_ops(&mut ops, image_id, img_w, img_h);

    // キャプション1行（下線のみ）
    let cap_x = center_x(caption, CAPTION_FONT_SIZE);
    fonts.caption.push_text_ops(&mut ops, caption, cap_x, CAPTION_Y, CAPTION_FONT_SIZE);
    add_rule_ops(&mut ops, RULE_BOTTOM_Y);

    PdfPage::new(Mm(297.0), Mm(210.0), ops)
}

/// 着手前竣工写真帳PDFを生成
pub fn generate_pair_pdf(
    pairs: &[PairEntry],
    project_name: &str,
    output_path: &Path,
) -> Result<()> {
    let doc_title = format!("着手前竣工_{}", project_name);
    let mut doc = PdfDocument::new(&doc_title);
    let fonts = load_fonts(&mut doc)?;

    // 表紙
    let mut pages = vec![build_cover_page(&fonts, project_name)];

    // 各ペア: 着手前→竣工の順
    let mut skipped = 0;
    let mut jpeg_map: std::collections::HashMap<String, (Vec<u8>, u32, u32)> = std::collections::HashMap::new();
    for (i, pair) in pairs.iter().enumerate() {
        eprintln!("[{}/{}] {}", i + 1, pairs.len(), pair.station_name);

        // 着手前
        let (before_id, before_w, before_h, before_jpeg) = match load_and_add_image(&mut doc, &pair.before_path) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("警告: 着手前写真読み込み失敗 ({}): {}", pair.before_path.display(), e);
                skipped += 1;
                continue;
            }
        };
        jpeg_map.insert(before_id.0.clone(), (before_jpeg, before_w, before_h));

        // 竣工
        let (after_id, after_w, after_h, after_jpeg) = match load_and_add_image(&mut doc, &pair.after_path) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("警告: 竣工写真読み込み失敗 ({}): {}", pair.after_path.display(), e);
                skipped += 1;
                continue;
            }
        };
        jpeg_map.insert(after_id.0.clone(), (after_jpeg, after_w, after_h));

        let before_caption = format!("着手前 - {}", pair.station_name);
        let after_caption = format!("竣工 - {}", pair.station_name);
        pages.push(build_photo_page(&fonts, &before_id, before_w, before_h, &before_caption));
        pages.push(build_photo_page(&fonts, &after_id, after_w, after_h, &after_caption));
    }

    if skipped > 0 {
        eprintln!("警告: {}ペアをスキップしました", skipped);
    }

    // 出力ディレクトリ確保
    if let Some(parent) = output_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    // フォントサブセット化で保存
    let save_options = PdfSaveOptions {
        subset_fonts: true,
        ..Default::default()
    };
    let mut warnings = Vec::new();
    let pdf_bytes = doc.with_pages(pages).save(&save_options, &mut warnings);

    for w in &warnings {
        let msg = format!("{:?}", w);
        if !msg.contains("Info") {
            eprintln!("PDF警告: {}", msg);
        }
    }

    // JPEG直接埋め込み（XObjectId名でマッチング）
    let pdf_bytes = replace_images_with_jpeg_by_name(pdf_bytes, &jpeg_map);

    eprintln!("PDF出力: {} ({:.1}MB)", output_path.display(), pdf_bytes.len() as f64 / 1_048_576.0);
    std::fs::write(output_path, &pdf_bytes)?;
    Ok(())
}

/// XObjectId名をキーにしてJPEG直接埋め込みに差し替える
fn replace_images_with_jpeg_by_name(
    pdf_bytes: Vec<u8>,
    jpeg_map: &std::collections::HashMap<String, (Vec<u8>, u32, u32)>,
) -> Vec<u8> {
    if jpeg_map.is_empty() {
        return pdf_bytes;
    }

    let mut doc = match lopdf::Document::load_mem(&pdf_bytes) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("lopdf parse失敗、JPEG最適化スキップ: {}", e);
            return pdf_bytes;
        }
    };

    let original_size = pdf_bytes.len();

    // XObject名 → ObjectId のマッピングを構築（任意のページのResourcesから）
    let pages = doc.get_pages();
    let mut name_to_id: std::collections::HashMap<String, lopdf::ObjectId> = std::collections::HashMap::new();

    if let Some((_, page_id)) = pages.iter().next() {
        if let Ok(page_obj) = doc.get_object(*page_id) {
            if let Ok(page_dict) = page_obj.as_dict() {
                if let Ok(res_ref) = page_dict.get(b"Resources") {
                    if let Ok((_, res_obj)) = doc.dereference(res_ref) {
                        if let Ok(res_dict) = res_obj.as_dict() {
                            if let Ok(xobj_ref) = res_dict.get(b"XObject") {
                                if let Ok((_, xobj_obj)) = doc.dereference(xobj_ref) {
                                    if let Ok(xobj_dict) = xobj_obj.as_dict() {
                                        for (name_bytes, obj_ref) in xobj_dict.iter() {
                                            let name = String::from_utf8_lossy(name_bytes).to_string();
                                            if let Ok((Some(resolved_id), _)) = doc.dereference(obj_ref) {
                                                name_to_id.insert(name, resolved_id);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut replaced = 0;
    for (xobj_name, (jpeg, w, h)) in jpeg_map {
        if let Some(obj_id) = name_to_id.get(xobj_name) {
            if let Some(obj) = doc.objects.get_mut(obj_id) {
                if let Ok(stream) = obj.as_stream_mut() {
                    stream.set_content(jpeg.clone());
                    stream.dict.set("Filter", lopdf::Object::Name(b"DCTDecode".to_vec()));
                    stream.dict.set("ColorSpace", lopdf::Object::Name(b"DeviceRGB".to_vec()));
                    stream.dict.set("BitsPerComponent", lopdf::Object::Integer(8));
                    stream.dict.set("Width", lopdf::Object::Integer(*w as i64));
                    stream.dict.set("Height", lopdf::Object::Integer(*h as i64));
                    stream.dict.remove(b"DecodeParms");
                    replaced += 1;
                }
            }
        }
    }

    let mut buf = Vec::new();
    if let Err(e) = doc.save_to(&mut buf) {
        eprintln!("lopdf save失敗、JPEG最適化スキップ: {}", e);
        return pdf_bytes;
    }

    eprintln!(
        "JPEG直接埋め込み: {}/{} 画像, {:.1}MB → {:.1}MB",
        replaced, jpeg_map.len(),
        original_size as f64 / 1_048_576.0,
        buf.len() as f64 / 1_048_576.0,
    );

    buf
}
