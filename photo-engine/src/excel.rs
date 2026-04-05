/// Excel generation using rust_xlsxwriter crate.
use serde::{Deserialize, Serialize};
use rust_xlsxwriter::*;
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
    match generate_excel_inner(&config) {
        Ok(result) => {
            let resp: EngineResponse<ExcelResult> = EngineResponse::success(result);
            to_json_string(&resp)
        }
        Err(e) => {
            let resp: EngineResponse<ExcelResult> = EngineResponse::failure(e);
            to_json_string(&resp)
        }
    }
}

fn generate_excel_inner(config: &ExcelConfig) -> Result<ExcelResult, String> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    if let Some(ref name) = config.sheet_name {
        worksheet.set_name(name)
            .map_err(|e| format!("failed to set sheet name: {}", e))?;
    }

    for (row_idx, row) in config.rows.iter().enumerate() {
        for (col_idx, cell) in row.iter().enumerate() {
            worksheet.write_string(row_idx as u32, col_idx as u16, cell)
                .map_err(|e| format!("failed to write cell ({},{}): {}", row_idx, col_idx, e))?;
        }
    }

    workbook.save(&config.output_path)
        .map_err(|e| format!("failed to save workbook: {}", e))?;

    Ok(ExcelResult {
        output_path: config.output_path.clone(),
    })
}
