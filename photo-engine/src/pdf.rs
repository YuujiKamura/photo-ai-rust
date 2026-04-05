/// PDF generation using printpdf 0.8 crate.
use serde::{Deserialize, Serialize};
use printpdf::*;
use crate::types::{EngineResponse, to_json_string};

#[derive(Deserialize, Debug)]
pub struct PdfConfig {
    pub output_path: String,
    pub photo_paths: Vec<String>,
    pub title: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct PdfResult {
    pub output_path: String,
}

/// A4 dimensions in pt
const A4_WIDTH_PT: f32 = 595.35;
const A4_HEIGHT_PT: f32 = 842.0;
const MARGIN_PT: f32 = 28.35;
const PHOTOS_PER_PAGE: usize = 3;

pub fn generate_pdf(config: PdfConfig) -> String {
    match generate_pdf_inner(&config) {
        Ok(result) => {
            let resp: EngineResponse<PdfResult> = EngineResponse::success(result);
            to_json_string(&resp)
        }
        Err(e) => {
            let resp: EngineResponse<PdfResult> = EngineResponse::failure(e);
            to_json_string(&resp)
        }
    }
}

fn generate_pdf_inner(config: &PdfConfig) -> Result<PdfResult, String> {
    let mut doc = PdfDocument::new(
        config.title.as_deref().unwrap_or("Photo Album"),
    );

    let usable_height = A4_HEIGHT_PT - MARGIN_PT * 2.0;
    let usable_width = A4_WIDTH_PT - MARGIN_PT * 2.0;
    let row_height = usable_height / PHOTOS_PER_PAGE as f32;

    let page_width_mm = Mm::from(Pt(A4_WIDTH_PT));
    let page_height_mm = Mm::from(Pt(A4_HEIGHT_PT));

    // Collect all images into pages
    let chunks: Vec<&[String]> = config.photo_paths.chunks(PHOTOS_PER_PAGE).collect();

    for (page_idx, chunk) in chunks.iter().enumerate() {
        let mut ops: Vec<Op> = Vec::new();

        // Title on first page
        if page_idx == 0 {
            if let Some(ref title) = config.title {
                ops.push(Op::StartTextSection);
                ops.push(Op::SetFontSizeBuiltinFont {
                    size: Pt(14.0),
                    font: BuiltinFont::Helvetica,
                });
                ops.push(Op::SetTextCursor {
                    pos: Point {
                        x: Pt(MARGIN_PT),
                        y: Pt(A4_HEIGHT_PT - MARGIN_PT + 5.0),
                    },
                });
                ops.push(Op::WriteTextBuiltinFont {
                    items: vec![TextItem::Text(title.clone())],
                    font: BuiltinFont::Helvetica,
                });
                ops.push(Op::EndTextSection);
            }
        }

        // Add images for this page
        for (slot, photo_path) in chunk.iter().enumerate() {
            let y_pt = A4_HEIGHT_PT - MARGIN_PT - ((slot + 1) as f32 * row_height);

            // Read and decode the image
            let image_bytes = std::fs::read(photo_path)
                .map_err(|e| format!("failed to read image {}: {}", photo_path, e))?;

            let dynamic_img = ::image::load_from_memory(&image_bytes)
                .map_err(|e| format!("failed to decode image {}: {}", photo_path, e))?;

            let img_w = dynamic_img.width() as f32;
            let img_h = dynamic_img.height() as f32;

            // Convert to RGB8 raw pixels
            let rgb_img = dynamic_img.to_rgb8();
            let raw_pixels = rgb_img.into_raw();
            let raw_image = RawImage {
                pixels: RawImageData::U8(raw_pixels),
                width: img_w as usize,
                height: img_h as usize,
                data_format: RawImageFormat::RGB8,
                tag: Vec::new(),
            };

            let image_id = doc.add_image(&raw_image);

            // Calculate scale to fit within slot
            let max_w = usable_width;
            let max_h = row_height - 4.0;

            // Image dimensions in pt (assume 72 DPI for PDF)
            let scale_x = max_w / img_w;
            let scale_y = max_h / img_h;
            let scale = scale_x.min(scale_y);

            let scaled_w = img_w * scale;
            let _scaled_h = img_h * scale;

            // Center horizontally within usable area
            let offset_x = MARGIN_PT + (max_w - scaled_w) / 2.0;

            ops.push(Op::UseXobject {
                id: image_id,
                transform: XObjectTransform {
                    translate_x: Some(Pt(offset_x)),
                    translate_y: Some(Pt(y_pt)),
                    scale_x: Some(scale),
                    scale_y: Some(scale),
                    ..Default::default()
                },
            });
        }

        let page = PdfPage::new(page_width_mm, page_height_mm, ops);
        doc.with_pages(vec![page]);
    }

    // Handle empty document (no photos)
    if config.photo_paths.is_empty() {
        let page = PdfPage::new(page_width_mm, page_height_mm, Vec::new());
        doc.with_pages(vec![page]);
    }

    // Save PDF
    let mut warnings = Vec::new();
    let pdf_bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
    std::fs::write(&config.output_path, &pdf_bytes)
        .map_err(|e| format!("failed to write PDF: {}", e))?;

    Ok(PdfResult {
        output_path: config.output_path.clone(),
    })
}
