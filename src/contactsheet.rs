//! コンタクトシート生成モジュール
//!
//! Before/After写真を番号付きグリッド画像にまとめる。
//! Pythonの`make_contact_sheet()`をRustに移植。

use crate::error::{PhotoAiError, Result};
use image::{DynamicImage, Rgb, RgbImage};
use imageproc::rect::Rect;
use log::{info, warn};
use std::path::{Path, PathBuf};

// --- 定数（Pythonと同一） ---
const CELL_W: u32 = 267;
const CELL_H: u32 = 200;
const LABEL_H: u32 = 22;
const GAP: u32 = 4;
const JPEG_QUALITY: u8 = 90;

// ラベルフォントサイズ（ビルトインラスタ描画）
const FONT_SCALE: f32 = 16.0;

pub struct ContactSheet {
    pub image_path: PathBuf,
    pub mapping: Vec<(u32, String)>, // [(1-based番号, ファイル名)]
}

/// 番号付きコンタクトシートをレンダリング（I/Oなし）
///
/// - `images`: (ファイル名, 読み込み済み画像) のスライス
/// - `prefix`: "B" or "A"
/// - `cols`: 列数
///
/// Returns: (レンダリング済みRgbImage, 番号→ファイル名マッピング)
pub fn render_contact_sheet(
    images: &[(String, DynamicImage)],
    prefix: &str,
    cols: u32,
) -> Result<(RgbImage, Vec<(u32, String)>)> {
    let n = images.len() as u32;
    if n == 0 {
        return Err(PhotoAiError::Config("コンタクトシート: 画像が0枚です".into()));
    }
    if cols == 0 {
        return Err(PhotoAiError::Config("コンタクトシート: 列数が0です".into()));
    }

    let rows = (n + cols - 1) / cols;
    let sheet_w = cols * CELL_W + (cols.saturating_sub(1)) * GAP;
    let sheet_h = rows * (CELL_H + LABEL_H) + (rows.saturating_sub(1)) * GAP;

    let mut sheet = RgbImage::from_pixel(sheet_w, sheet_h, Rgb([255, 255, 255]));
    let mut mapping = Vec::new();

    // フォント読み込み
    let font = load_font();

    for (i, (fname, img)) in images.iter().enumerate() {
        let num = (i as u32) + 1;
        let label = format!("{}{:02}", prefix, num);
        mapping.push((num, fname.clone()));

        let col = i as u32 % cols;
        let row = i as u32 / cols;
        let x = col * (CELL_W + GAP);
        let y = row * (CELL_H + LABEL_H + GAP);

        // ラベル帯（黒背景）
        imageproc::drawing::draw_filled_rect_mut(
            &mut sheet,
            Rect::at(x as i32, y as i32).of_size(CELL_W, LABEL_H),
            Rgb([0, 0, 0]),
        );

        // ラベルテキスト（白文字）
        draw_label(&mut sheet, &font, &label, x + 4, y + 2);

        // サムネイル（generate_contact_sheetで既にリサイズ済み、外部呼び出し時はここでリサイズ）
        let thumb = if img.width() == CELL_W && img.height() == CELL_H {
            img.to_rgb8()
        } else {
            resize_with_padding(img, CELL_W, CELL_H).to_rgb8()
        };
        let thumb_rgb = thumb;
        image::imageops::overlay(&mut sheet, &thumb_rgb, x as i64, (y + LABEL_H) as i64);
    }

    Ok((sheet, mapping))
}

/// 番号付きコンタクトシートを生成（I/Oラッパー）
///
/// - `image_files`: 入力画像パスのスライス（ソート済み）
/// - `prefix`: "B" or "A"
/// - `cols`: 列数（Before=5, After=7）
/// - `output_path`: 出力JPEG先
pub fn generate_contact_sheet(
    image_files: &[PathBuf],
    prefix: &str,
    cols: u32,
    output_path: &Path,
) -> Result<ContactSheet> {
    // 画像読み込み（即座にサムネイルサイズにリサイズしてメモリ節約）
    let mut images: Vec<(String, DynamicImage)> = Vec::new();
    for fpath in image_files {
        let fname = fpath
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        match image::open(fpath) {
            Ok(img) => {
                let resized = resize_with_padding(&img, CELL_W, CELL_H);
                images.push((fname, resized));
            }
            Err(e) => {
                warn!("cannot load {}: {}", fpath.display(), e);
            }
        }
    }

    if images.len() < image_files.len() {
        warn!(
            "{}/{} images loaded ({} skipped)",
            images.len(),
            image_files.len(),
            image_files.len() - images.len()
        );
    }

    // レンダリング
    let (sheet, mapping) = render_contact_sheet(&images, prefix, cols)?;

    // JPEG保存
    let sheet_w = sheet.width();
    let sheet_h = sheet.height();
    let dynamic = DynamicImage::ImageRgb8(sheet);
    let mut buf = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, JPEG_QUALITY);
    dynamic
        .write_with_encoder(encoder)
        .map_err(|e| PhotoAiError::ImageLoad(format!("コンタクトシートJPEG保存エラー: {}", e)))?;
    std::fs::write(output_path, &buf)?;

    info!(
        "Generated: {} ({}x{}, {} images)",
        output_path.display(),
        sheet_w,
        sheet_h,
        images.len()
    );

    Ok(ContactSheet {
        image_path: output_path.to_path_buf(),
        mapping,
    })
}

// --- フォント読み込み ---

enum FontData {
    AbGlyph(ab_glyph::FontArc),
    None,
}

fn load_font() -> FontData {
    for path in font_candidates() {
        if let Some(font) = try_load_font(Path::new(&path)) {
            return font;
        }
    }
    FontData::None
}

fn font_candidates() -> Vec<String> {
    let mut paths = Vec::new();
    #[cfg(windows)]
    {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
        paths.push(format!("{}\\Fonts\\arial.ttf", windir));
    }
    paths.push("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string());
    paths.push("/usr/share/fonts/TTF/DejaVuSans.ttf".to_string());
    paths
}

fn try_load_font(path: &Path) -> Option<FontData> {
    if !path.exists() {
        return None;
    }
    let data = std::fs::read(path).ok()?;
    let font = ab_glyph::FontArc::try_from_vec(data).ok()?;
    Some(FontData::AbGlyph(font))
}

// --- 画像リサイズヘルパー ---

/// アスペクト比を維持してリサイズし、余白を白で埋める
fn resize_with_padding(img: &DynamicImage, target_w: u32, target_h: u32) -> DynamicImage {
    let resized = img.resize(target_w, target_h, image::imageops::FilterType::Lanczos3);
    if resized.width() == target_w && resized.height() == target_h {
        return resized;
    }
    let mut canvas = RgbImage::from_pixel(target_w, target_h, Rgb([255, 255, 255]));
    let offset_x = (target_w.saturating_sub(resized.width())) / 2;
    let offset_y = (target_h.saturating_sub(resized.height())) / 2;
    image::imageops::overlay(&mut canvas, &resized.to_rgb8(), offset_x as i64, offset_y as i64);
    DynamicImage::ImageRgb8(canvas)
}

// --- 描画ヘルパー ---

fn draw_label(img: &mut RgbImage, font: &FontData, text: &str, x: u32, y: u32) {
    match font {
        FontData::AbGlyph(f) => {
            let scale = ab_glyph::PxScale::from(FONT_SCALE);
            imageproc::drawing::draw_text_mut(
                img,
                Rgb([255, 255, 255]),
                x as i32,
                y as i32,
                scale,
                f,
                text,
            );
        }
        FontData::None => {
            // フォントなし: 簡易ラスタ描画（最低限の数字・英字）
            draw_simple_text(img, text, x, y);
        }
    }
}

/// フォントなしフォールバック: 1px幅の簡易描画
fn draw_simple_text(img: &mut RgbImage, text: &str, x: u32, y: u32) {
    let white = Rgb([255, 255, 255]);
    let mut cx = x;
    for ch in text.chars() {
        let pattern = char_pattern(ch);
        for (dy, row) in pattern.iter().enumerate() {
            for (dx, &pixel) in row.iter().enumerate() {
                if pixel == 1 {
                    let px = cx + dx as u32;
                    let py = y + dy as u32;
                    if px < img.width() && py < img.height() {
                        img.put_pixel(px, py, white);
                    }
                }
            }
        }
        cx += pattern[0].len() as u32 + 1; // 1px spacing
    }
}

// --- 簡易ビットマップフォントパターン（static, zero-allocation） ---

static PATTERN_0: [[u8; 5]; 7] = [
    [0,1,1,1,0], [1,0,0,0,1], [1,0,0,1,1], [1,0,1,0,1],
    [1,1,0,0,1], [1,0,0,0,1], [0,1,1,1,0],
];
static PATTERN_1: [[u8; 5]; 7] = [
    [0,0,1,0,0], [0,1,1,0,0], [0,0,1,0,0], [0,0,1,0,0],
    [0,0,1,0,0], [0,0,1,0,0], [0,1,1,1,0],
];
static PATTERN_2: [[u8; 5]; 7] = [
    [0,1,1,1,0], [1,0,0,0,1], [0,0,0,1,0], [0,0,1,0,0],
    [0,1,0,0,0], [1,0,0,0,0], [1,1,1,1,1],
];
static PATTERN_3: [[u8; 5]; 7] = [
    [0,1,1,1,0], [1,0,0,0,1], [0,0,0,0,1], [0,0,1,1,0],
    [0,0,0,0,1], [1,0,0,0,1], [0,1,1,1,0],
];
static PATTERN_4: [[u8; 5]; 7] = [
    [0,0,0,1,0], [0,0,1,1,0], [0,1,0,1,0], [1,0,0,1,0],
    [1,1,1,1,1], [0,0,0,1,0], [0,0,0,1,0],
];
static PATTERN_5: [[u8; 5]; 7] = [
    [1,1,1,1,1], [1,0,0,0,0], [1,1,1,1,0], [0,0,0,0,1],
    [0,0,0,0,1], [1,0,0,0,1], [0,1,1,1,0],
];
static PATTERN_6: [[u8; 5]; 7] = [
    [0,1,1,1,0], [1,0,0,0,0], [1,1,1,1,0], [1,0,0,0,1],
    [1,0,0,0,1], [1,0,0,0,1], [0,1,1,1,0],
];
static PATTERN_7: [[u8; 5]; 7] = [
    [1,1,1,1,1], [0,0,0,0,1], [0,0,0,1,0], [0,0,1,0,0],
    [0,0,1,0,0], [0,0,1,0,0], [0,0,1,0,0],
];
static PATTERN_8: [[u8; 5]; 7] = [
    [0,1,1,1,0], [1,0,0,0,1], [1,0,0,0,1], [0,1,1,1,0],
    [1,0,0,0,1], [1,0,0,0,1], [0,1,1,1,0],
];
static PATTERN_9: [[u8; 5]; 7] = [
    [0,1,1,1,0], [1,0,0,0,1], [1,0,0,0,1], [0,1,1,1,1],
    [0,0,0,0,1], [0,0,0,0,1], [0,1,1,1,0],
];
static PATTERN_A: [[u8; 5]; 7] = [
    [0,0,1,0,0], [0,1,0,1,0], [1,0,0,0,1], [1,1,1,1,1],
    [1,0,0,0,1], [1,0,0,0,1], [1,0,0,0,1],
];
static PATTERN_B: [[u8; 5]; 7] = [
    [1,1,1,1,0], [1,0,0,0,1], [1,0,0,0,1], [1,1,1,1,0],
    [1,0,0,0,1], [1,0,0,0,1], [1,1,1,1,0],
];
static PATTERN_BLANK: [[u8; 5]; 7] = [
    [0,0,0,0,0], [0,0,0,0,0], [0,0,0,0,0], [0,0,0,0,0],
    [0,0,0,0,0], [0,0,0,0,0], [0,0,0,0,0],
];

/// 5x7の簡易ビットマップフォント（数字とA/B用）— static参照で零アロケーション
fn char_pattern(ch: char) -> &'static [[u8; 5]; 7] {
    match ch {
        '0' => &PATTERN_0,
        '1' => &PATTERN_1,
        '2' => &PATTERN_2,
        '3' => &PATTERN_3,
        '4' => &PATTERN_4,
        '5' => &PATTERN_5,
        '6' => &PATTERN_6,
        '7' => &PATTERN_7,
        '8' => &PATTERN_8,
        '9' => &PATTERN_9,
        'A' => &PATTERN_A,
        'B' => &PATTERN_B,
        _ => &PATTERN_BLANK,
    }
}
