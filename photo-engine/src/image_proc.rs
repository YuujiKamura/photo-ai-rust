/// Image processing using image and imageproc crates.
use serde::{Deserialize, Serialize};
use image::{DynamicImage, Rgb, RgbImage};
use crate::types::{EngineResponse, to_json_string};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ImageOperation {
    Resize { width: u32, height: u32 },
    ContactSheet { columns: u32, thumb_width: u32, thumb_height: u32 },
    Thumbnail { max_size: u32 },
}

#[derive(Deserialize, Debug)]
pub struct ImageConfig {
    pub input_paths: Vec<String>,
    pub output_path: String,
    pub operation: ImageOperation,
}

#[derive(Serialize, Debug)]
pub struct ImageResult {
    pub output_path: String,
    pub width: u32,
    pub height: u32,
}

const JPEG_QUALITY: u8 = 90;

pub fn process_image(config: ImageConfig) -> String {
    match process_image_inner(&config) {
        Ok(result) => {
            let resp: EngineResponse<ImageResult> = EngineResponse::success(result);
            to_json_string(&resp)
        }
        Err(e) => {
            let resp: EngineResponse<ImageResult> = EngineResponse::failure(e);
            to_json_string(&resp)
        }
    }
}

fn process_image_inner(config: &ImageConfig) -> Result<ImageResult, String> {
    match &config.operation {
        ImageOperation::Resize { width, height } => {
            do_resize(config, *width, *height)
        }
        ImageOperation::Thumbnail { max_size } => {
            do_thumbnail(config, *max_size)
        }
        ImageOperation::ContactSheet { columns, thumb_width, thumb_height } => {
            do_contact_sheet(config, *columns, *thumb_width, *thumb_height)
        }
    }
}

fn do_resize(config: &ImageConfig, width: u32, height: u32) -> Result<ImageResult, String> {
    let input = config.input_paths.first()
        .ok_or_else(|| "no input path provided".to_string())?;
    let img = image::open(input)
        .map_err(|e| format!("failed to open image: {}", e))?;
    let resized = img.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
    save_jpeg(&resized, &config.output_path)?;
    Ok(ImageResult {
        output_path: config.output_path.clone(),
        width,
        height,
    })
}

fn do_thumbnail(config: &ImageConfig, max_size: u32) -> Result<ImageResult, String> {
    let input = config.input_paths.first()
        .ok_or_else(|| "no input path provided".to_string())?;
    let img = image::open(input)
        .map_err(|e| format!("failed to open image: {}", e))?;
    let thumb = img.resize(max_size, max_size, image::imageops::FilterType::Lanczos3);
    let w = thumb.width();
    let h = thumb.height();
    save_jpeg(&thumb, &config.output_path)?;
    Ok(ImageResult {
        output_path: config.output_path.clone(),
        width: w,
        height: h,
    })
}

fn do_contact_sheet(
    config: &ImageConfig,
    columns: u32,
    thumb_width: u32,
    thumb_height: u32,
) -> Result<ImageResult, String> {
    if config.input_paths.is_empty() {
        return Err("no input paths for contact sheet".to_string());
    }
    if columns == 0 {
        return Err("columns must be > 0".to_string());
    }

    let n = config.input_paths.len() as u32;
    let rows = (n + columns - 1) / columns;
    let sheet_w = columns * thumb_width;
    let sheet_h = rows * thumb_height;

    let mut sheet = RgbImage::from_pixel(sheet_w, sheet_h, Rgb([255, 255, 255]));

    for (i, path) in config.input_paths.iter().enumerate() {
        let img = match image::open(path) {
            Ok(img) => img,
            Err(_) => continue,
        };
        let thumb = resize_with_padding(&img, thumb_width, thumb_height);
        let col = i as u32 % columns;
        let row = i as u32 / columns;
        let x = col * thumb_width;
        let y = row * thumb_height;
        image::imageops::overlay(&mut sheet, &thumb.to_rgb8(), x as i64, y as i64);
    }

    let dynamic = DynamicImage::ImageRgb8(sheet);
    save_jpeg(&dynamic, &config.output_path)?;

    Ok(ImageResult {
        output_path: config.output_path.clone(),
        width: sheet_w,
        height: sheet_h,
    })
}

/// Resize maintaining aspect ratio, pad with white
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

fn save_jpeg(img: &DynamicImage, path: &str) -> Result<(), String> {
    let mut buf = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, JPEG_QUALITY);
    img.write_with_encoder(encoder)
        .map_err(|e| format!("failed to encode JPEG: {}", e))?;
    std::fs::write(path, &buf)
        .map_err(|e| format!("failed to write file: {}", e))?;
    Ok(())
}
