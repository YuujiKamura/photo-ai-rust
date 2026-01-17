use crate::analyzer::AnalysisResult;
use crate::error::{PhotoAiError, Result};
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

const A4_WIDTH_MM: f32 = 210.0;
const A4_HEIGHT_MM: f32 = 297.0;
const MARGIN_MM: f32 = 10.0;

pub fn generate_pdf(
    results: &[AnalysisResult],
    output_path: &Path,
    _photos_per_page: u8,
    title: &str,
) -> Result<()> {
    let (doc, page1, layer1) = PdfDocument::new(
        title,
        Mm(A4_WIDTH_MM),
        Mm(A4_HEIGHT_MM),
        "Layer 1",
    );

    let current_layer = doc.get_page(page1).get_layer(layer1);

    // TODO: 日本語フォント埋め込み
    // TODO: 写真配置
    // TODO: テキスト配置

    // 仮のテキスト出力
    let font = doc.add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| PhotoAiError::PdfGeneration(format!("フォント追加エラー: {:?}", e)))?;

    current_layer.use_text(
        format!("{} - {} photos", title, results.len()),
        12.0,
        Mm(MARGIN_MM),
        Mm(A4_HEIGHT_MM - MARGIN_MM - 10.0),
        &font,
    );

    // 保存
    let file = File::create(output_path)?;
    let writer = BufWriter::new(file);
    doc.save(&mut BufWriter::new(writer))
        .map_err(|e| PhotoAiError::PdfGeneration(format!("PDF保存エラー: {:?}", e)))?;

    Ok(())
}
