/// PDF generation stub.
/// TODO: Implement from src/export/pdf.rs and src/export/pair_pdf.rs in parent crate.
use serde::{Deserialize, Serialize};
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

pub fn generate_pdf(config: PdfConfig) -> String {
    // TODO: Implement using printpdf crate.
    // Reference: src/export/pdf.rs, src/export/pair_pdf.rs
    let _ = config;
    let resp: EngineResponse<PdfResult> = EngineResponse::failure("not yet implemented");
    to_json_string(&resp)
}
