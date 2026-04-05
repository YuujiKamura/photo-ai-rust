/// Excel generation stub.
/// TODO: Implement from src/export/excel.rs in parent crate.
use serde::{Deserialize, Serialize};
use crate::types::{EngineResponse, to_json_string};

#[derive(Deserialize, Debug)]
pub struct ExcelConfig {
    pub output_path: String,
    pub rows: Vec<Vec<String>>,
    pub sheet_name: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct ExcelResult {
    pub output_path: String,
}

pub fn generate_excel(config: ExcelConfig) -> String {
    // TODO: Implement using rust_xlsxwriter crate.
    // Reference: src/export/excel.rs
    let _ = config;
    let resp: EngineResponse<ExcelResult> = EngineResponse::failure("not yet implemented");
    to_json_string(&resp)
}
