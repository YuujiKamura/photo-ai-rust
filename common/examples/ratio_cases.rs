use photo_ai_common::layout::{
    excel_width_to_px, ExcelLayout, PHOTO_COL_WIDTH, PHOTO_ROWS, PT_TO_PX,
};
use rust_xlsxwriter::{Format, FormatBorder, Image, Workbook, XlsxError};
use std::env;
use std::path::{Path, PathBuf};

fn main() -> Result<(), XlsxError> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: ratio_cases <image_path> [output_path]");
        std::process::exit(1);
    }

    let image_path = Path::new(&args[1]);
    if !image_path.exists() {
        eprintln!("Image not found: {}", image_path.display());
        std::process::exit(1);
    }

    let output_path = if args.len() >= 3 {
        PathBuf::from(&args[2])
    } else {
        let parent = image_path.parent().unwrap_or_else(|| Path::new("."));
        let stem = image_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("ratio_cases");
        parent.join(format!("{stem}_ratio_cases.xlsx"))
    };

    let layout = ExcelLayout::three_up();
    let photo_rows = PHOTO_ROWS as u32;
    let row_height_px = (layout.row_height_pt * PT_TO_PX).round() as u32;
    let target_width_px = excel_width_to_px(PHOTO_COL_WIDTH) as u32;
    let target_height_px = row_height_px * photo_rows;

    let mut workbook = Workbook::new();
    let border = Format::new().set_border(FormatBorder::Thin);

    let image = Image::new(image_path)?;

    let cases = [
        ("fit_to_cell_keep_false", Case::FitToCell { keep_aspect: false }),
        ("fit_to_cell_keep_true", Case::FitToCell { keep_aspect: true }),
        ("fit_to_cell_centered", Case::FitToCellCentered),
        ("scale_to_size_keep_false", Case::ScaleToSize { keep_aspect: false }),
        ("scale_to_size_keep_true", Case::ScaleToSize { keep_aspect: true }),
        ("single_scale_centered", Case::SingleScaleCentered),
        ("single_scale_no_offset", Case::SingleScaleNoOffset),
    ];

    for (sheet_name, case) in cases {
        let worksheet = workbook.add_worksheet();
        worksheet.set_name(sheet_name)?;

        worksheet.set_column_width_pixels(0, target_width_px)?;
        for r in 0..photo_rows {
            worksheet.set_row_height_pixels(r, row_height_px)?;
        }

        // Visual border for the intended photo area.
        worksheet.merge_range(0, 0, photo_rows - 1, 0, "", &border)?;

        match case {
            Case::FitToCell { keep_aspect } => {
                worksheet.insert_image_fit_to_cell(0, 0, &image, keep_aspect)?;
            }
            Case::FitToCellCentered => {
                worksheet.insert_image_fit_to_cell_centered(0, 0, &image)?;
            }
            Case::ScaleToSize { keep_aspect } => {
                let scaled = image.clone().set_scale_to_size(
                    target_width_px,
                    target_height_px,
                    keep_aspect,
                );
                worksheet.insert_image(0, 0, &scaled)?;
            }
            Case::SingleScaleCentered => {
                let k = (target_width_px as f64 / image.width())
                    .min(target_height_px as f64 / image.height());
                let scaled_width = image.width() * k;
                let scaled_height = image.height() * k;
                let x_offset = ((target_width_px as f64 - scaled_width) / 2.0)
                    .round()
                    .max(0.0) as u32;
                let y_offset = ((target_height_px as f64 - scaled_height) / 2.0)
                    .round()
                    .max(0.0) as u32;
                let scaled = image.clone().set_scale_width(k).set_scale_height(k);
                worksheet.insert_image_with_offset(0, 0, &scaled, x_offset, y_offset)?;
            }
            Case::SingleScaleNoOffset => {
                let k = (target_width_px as f64 / image.width())
                    .min(target_height_px as f64 / image.height());
                let scaled = image.clone().set_scale_width(k).set_scale_height(k);
                worksheet.insert_image(0, 0, &scaled)?;
            }
        }
    }

    workbook.save(&output_path)?;
    println!("Wrote: {}", output_path.display());
    println!(
        "Target area: {}px x {}px (rows: {}, row_height_px: {})",
        target_width_px, target_height_px, photo_rows, row_height_px
    );
    println!(
        "Image size: {}x{} px (dpi: {}x{})",
        image.width(),
        image.height(),
        image.width_dpi(),
        image.height_dpi()
    );

    Ok(())
}

enum Case {
    FitToCell { keep_aspect: bool },
    FitToCellCentered,
    ScaleToSize { keep_aspect: bool },
    SingleScaleCentered,
    SingleScaleNoOffset,
}
