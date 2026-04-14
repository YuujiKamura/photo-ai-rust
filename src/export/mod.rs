pub mod pdf;
pub mod pair_pdf;
pub mod excel;
pub mod photo_xml;

use crate::analyzer::AnalysisResult;
use crate::export_types::{ExportFormat, PdfQuality};
use crate::error::Result;
use photo_ai_common::export::pdf_embed::embed_analysis_to_pdf;
use std::path::Path;

fn output_path_for_format(output: &Path, title: &str, extension: &str) -> std::path::PathBuf {
    if output.is_dir() || output.extension().is_none() {
        // ディレクトリまたは拡張子なし: title + extension で生成
        output.join(format!("{}.{}", title, extension))
    } else {
        // ファイルパス指定: 親ディレクトリ + ステム + 指定拡張子
        let parent = output.parent().unwrap_or_else(|| Path::new("."));
        let stem = output
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(title);
        parent.join(format!("{}.{}", stem, extension))
    }
}

fn output_paths_for_both(output: &Path, title: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let pdf_path = output_path_for_format(output, title, "pdf");
    let excel_path = output_path_for_format(output, title, "xlsx");
    (pdf_path, excel_path)
}

pub fn export_results(
    results: &[AnalysisResult],
    format: &ExportFormat,
    output_dir: &Path,
    photos_per_page: u8,
    title: &str,
    pdf_quality: PdfQuality,
) -> Result<()> {
    // skip=trueの写真を除外
    let active: Vec<AnalysisResult> = results.iter().filter(|r| !r.skip).cloned().collect();
    let skipped = results.len() - active.len();
    if skipped > 0 {
        println!("  スキップ除外: {}枚", skipped);
    }
    let results = &active;

    match format {
        ExportFormat::Pdf => {
            let output_path = output_path_for_format(output_dir, title, "pdf");
            println!("- PDFを生成中... (品質: {})", pdf_quality);
            pdf::generate_pdf(results, &output_path, photos_per_page, title, pdf_quality)?;
            // 解析結果をPDFに埋め込み
            if let Err(e) = embed_analysis_to_pdf(&output_path, results) {
                eprintln!("警告: PDF埋め込みに失敗: {}", e);
            } else {
                println!("  解析結果をPDFに埋め込みました");
            }
            println!("✔ PDF出力: {}", output_path.display());
        }
        ExportFormat::Excel => {
            let output_path = output_path_for_format(output_dir, title, "xlsx");
            println!("- Excelを生成中...");
            excel::generate_excel_with_options(results, &output_path, title, photos_per_page)?;
            println!("✔ Excel出力: {}", output_path.display());
        }
        ExportFormat::PhotoXml => {
            println!("- PHOTO.XMLを生成中...");
            let xml_path = photo_xml::generate_photo_xml(results, output_dir)?;
            println!("✔ PHOTO.XML出力: {}", xml_path.display());
        }
        ExportFormat::Both => {
            let (pdf_path, excel_path) = output_paths_for_both(output_dir, title);

            println!("- PDFを生成中... (品質: {})", pdf_quality);
            pdf::generate_pdf(results, &pdf_path, photos_per_page, title, pdf_quality)?;
            // 解析結果をPDFに埋め込み
            if let Err(e) = embed_analysis_to_pdf(&pdf_path, results) {
                eprintln!("警告: PDF埋め込みに失敗: {}", e);
            } else {
                println!("  解析結果をPDFに埋め込みました");
            }
            println!("✔ PDF出力: {}", pdf_path.display());

            println!("- Excelを生成中...");
            excel::generate_excel_with_options(results, &excel_path, title, photos_per_page)?;
            println!("✔ Excel出力: {}", excel_path.display());
        }
    }

    Ok(())
}
