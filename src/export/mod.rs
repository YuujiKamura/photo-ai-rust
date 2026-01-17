pub mod pdf;
pub mod excel;
pub mod layout;

use crate::analyzer::AnalysisResult;
use crate::cli::{ExportFormat, PdfQuality};
use crate::error::Result;
use std::path::Path;

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
            let output_path = output_dir.join(format!("{}.pdf", title));
            println!("- PDFを生成中... (品質: {})", pdf_quality);
            pdf::generate_pdf(results, &output_path, photos_per_page, title, pdf_quality)?;
            println!("✔ PDF出力: {}", output_path.display());
        }
        ExportFormat::Excel => {
            let output_path = output_dir.join(format!("{}.xlsx", title));
            println!("- Excelを生成中...");
            excel::generate_excel(results, &output_path, title)?;
            println!("✔ Excel出力: {}", output_path.display());
        }
        ExportFormat::Both => {
            let pdf_path = output_dir.join(format!("{}.pdf", title));
            let excel_path = output_dir.join(format!("{}.xlsx", title));

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
