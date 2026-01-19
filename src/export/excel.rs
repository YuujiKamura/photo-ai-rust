//! Excel生成（CLI版）
//!
//! ExcelJSブリッジ（Node.js）を使用

use crate::error::{PhotoAiError, Result};
use photo_ai_common::AnalysisResult;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsPhoto {
    file_name: String,
    file_path: String,
    date: String,
    photo_category: String,
    work_type: String,
    variety: String,
    detail: String,
    station: String,
    remarks: String,
    measurements: String,
}

pub fn generate_excel(
    results: &[AnalysisResult],
    output_path: &Path,
    title: &str,
) -> Result<()> {
    generate_excel_with_options(results, output_path, title, 3)
}

pub fn generate_excel_with_options(
    results: &[AnalysisResult],
    output_path: &Path,
    title: &str,
    photos_per_page: u8,
) -> Result<()> {
    generate_excel_via_exceljs(results, output_path, title, photos_per_page)?;

    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsOptions {
    title: String,
    photos_per_page: u8,
}

#[derive(Serialize)]
struct JsPayload {
    photos: Vec<JsPhoto>,
    options: JsOptions,
}

fn generate_excel_via_exceljs(
    results: &[AnalysisResult],
    output_path: &Path,
    title: &str,
    photos_per_page: u8,
) -> Result<()> {
    let photos = results
        .iter()
        .map(|r| JsPhoto {
            file_name: r.file_name.clone(),
            file_path: r.file_path.clone(),
            date: r.date.clone(),
            photo_category: r.photo_category.clone(),
            work_type: r.work_type.clone(),
            variety: r.variety.clone(),
            detail: r.detail.clone(),
            station: r.station.clone(),
            remarks: r.remarks.clone(),
            measurements: r.measurements.clone(),
        })
        .collect();

    let payload = JsPayload {
        photos,
        options: JsOptions {
            title: title.to_string(),
            photos_per_page,
        },
    };

    let mut temp_path = std::env::temp_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| PhotoAiError::ExcelGeneration(format!("時刻取得エラー: {}", e)))?
        .as_millis();
    temp_path.push(format!("photo-ai-exceljs-{}.json", stamp));

    let json = serde_json::to_vec_pretty(&payload)
        .map_err(|e| PhotoAiError::ExcelGeneration(format!("JSON生成エラー: {}", e)))?;
    let mut file = std::fs::File::create(&temp_path)
        .map_err(|e| PhotoAiError::ExcelGeneration(format!("JSON保存エラー: {}", e)))?;
    file.write_all(&json)
        .map_err(|e| PhotoAiError::ExcelGeneration(format!("JSON書き込みエラー: {}", e)))?;

    let script_path = exceljs_script_path();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&temp_path)
        .arg(output_path)
        .output()
        .map_err(|e| PhotoAiError::ExcelGeneration(format!("Node起動エラー: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PhotoAiError::ExcelGeneration(format!(
            "ExcelJS失敗: {}",
            stderr.trim()
        )));
    }

    Ok(())
}

fn exceljs_script_path() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.join("web-wasm").join("js").join("excel-bridge.js")
}
