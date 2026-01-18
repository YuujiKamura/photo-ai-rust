pub mod pdf;
pub mod excel;

use crate::analyzer::AnalysisResult;
use crate::cli::{ExportFormat, PdfQuality};
use crate::error::Result;
use std::path::Path;

fn output_path_for_format(output: &Path, title: &str, extension: &str) -> std::path::PathBuf {
    if output.is_dir() || output.extension().is_none() {
        output.join(format!("{}.{}", title, extension))
    } else {
        output.to_path_buf()
    }
}

fn output_paths_for_both(output: &Path, title: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    if output.is_dir() || output.extension().is_none() {
        let pdf_path = output.join(format!("{}.pdf", title));
        let excel_path = output.join(format!("{}.xlsx", title));
        (pdf_path, excel_path)
    } else {
        let parent = output.parent().unwrap_or_else(|| Path::new("."));
        let stem = output
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(title);
        let pdf_path = parent.join(format!("{}.pdf", stem));
        let excel_path = parent.join(format!("{}.xlsx", stem));
        (pdf_path, excel_path)
    }
}

pub fn export_results(
    results: &[AnalysisResult],
    format: &ExportFormat,
    output_dir: &Path,
    photos_per_page: u8,
    title: &str,
    pdf_quality: PdfQuality,
) -> Result<()> {
    match format {
        ExportFormat::Pdf => {
            let output_path = output_path_for_format(output_dir, title, "pdf");
            println!("- PDFを生成中... (品質: {})", pdf_quality);
            pdf::generate_pdf(results, &output_path, photos_per_page, title, pdf_quality)?;
            println!("✔ PDF出力: {}", output_path.display());
        }
        ExportFormat::Excel => {
            let output_path = output_path_for_format(output_dir, title, "xlsx");
            println!("- Excelを生成中...");
            excel::generate_excel(results, &output_path, title)?;
            println!("✔ Excel出力: {}", output_path.display());
        }
        ExportFormat::Both => {
            let (pdf_path, excel_path) = output_paths_for_both(output_dir, title);

            println!("- PDFを生成中... (品質: {})", pdf_quality);
            pdf::generate_pdf(results, &pdf_path, photos_per_page, title, pdf_quality)?;
            println!("✔ PDF出力: {}", pdf_path.display());

            println!("- Excelを生成中...");
            excel::generate_excel(results, &excel_path, title)?;
            println!("✔ Excel出力: {}", excel_path.display());
        }
    }

    Ok(())
}
