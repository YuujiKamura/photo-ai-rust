use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::error::Result;
use crate::cli::PdfQuality;
use photo_ai_common::AnalysisResult;

/// DLLレスポンスの共通ヘッダー
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct DllResponseHeader {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    error: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PDFConfig {
    input_json: String,       // 解析済み結果JSONのパス
    output_path: String,      // 出力先パス (Go側: outputPath)
    photos_per_page: usize,
    quality: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PDFResult {
    #[serde(flatten)]
    header: DllResponseHeader,
    output_path: String,
    page_count: usize,
}

#[no_mangle]
pub extern "C" fn photo_engine_generate_pdf(req_json: *const c_char, out_buf: *mut c_char, out_len: usize) -> i32 {
    let req = match parse_req::<PDFConfig>(req_json) {
        Ok(r) => r,
        Err(e) => return write_err(e, out_buf, out_len),
    };

    let result = (|| -> Result<PDFResult> {
        // 1. JSONから解析結果を読み込む
        let json_data = std::fs::read_to_string(&req.input_json)?;
        let photos: Vec<AnalysisResult> = serde_json::from_str(&json_data)
            .map_err(|e| crate::error::PhotoAiError::MasterLoad(format!("JSON parse error: {}", e)))?;

        // 2. 出力パスの決定
        let output_path = if req.output_path.is_empty() {
            Path::new(&req.input_json).with_extension("pdf")
        } else {
            PathBuf::from(&req.output_path)
        };

        // 3. 品質のパース
        let quality = match req.quality.as_str() {
            "high" => PdfQuality::High,
            "low" => PdfQuality::Low,
            _ => PdfQuality::Medium,
        };

        // 4. RustのPDF生成ロジック呼び出し
        crate::export::pdf::generate_pdf(
            &photos,
            &output_path,
            req.photos_per_page as u8,
            "写真台帳",
            quality,
        )?;

        let page_count = photos.len().div_ceil(req.photos_per_page);

        Ok(PDFResult {
            header: DllResponseHeader::default(),
            output_path: output_path.display().to_string(),
            page_count,
        })
    })();

    match result {
        Ok(res) => write_resp(res, out_buf, out_len),
        Err(e) => write_err(e.to_string(), out_buf, out_len),
    }
}

// 他の stub (Go側がFindProcでエラーにならないよう空実装)
#[no_mangle] pub extern "C" fn photo_engine_generate_excel(_: *const c_char, _: *mut c_char, _: usize) -> i32 { 0 }
#[no_mangle] pub extern "C" fn photo_engine_process_image(_: *const c_char, _: *mut c_char, _: usize) -> i32 { 0 }
#[no_mangle] pub extern "C" fn photo_engine_extract_exif(_: *const c_char, _: *mut c_char, _: usize) -> i32 { 0 }

// --- Helpers ---

fn parse_req<T: for<'de> Deserialize<'de>>(req_json: *const c_char) -> std::result::Result<T, String> {
    if req_json.is_null() {
        return Err("null request".to_string());
    }
    let c_str = unsafe { CStr::from_ptr(req_json) };
    let json_str = c_str.to_str().map_err(|e| e.to_string())?;
    serde_json::from_str(json_str).map_err(|e| e.to_string())
}

fn write_resp<T: Serialize>(resp: T, out_buf: *mut c_char, out_len: usize) -> i32 {
    let json = serde_json::to_string(&resp).unwrap();
    write_to_buf(&json, out_buf, out_len)
}

fn write_err(msg: String, out_buf: *mut c_char, out_len: usize) -> i32 {
    let resp = DllResponseHeader { error: msg };
    let json = serde_json::to_string(&resp).unwrap();
    write_to_buf(&json, out_buf, out_len)
}

fn write_to_buf(json: &str, out_buf: *mut c_char, out_len: usize) -> i32 {
    let bytes = json.as_bytes();
    if bytes.len() > out_len {
        return -(bytes.len() as i32);
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf as *mut u8, bytes.len());
    }
    bytes.len() as i32
}
