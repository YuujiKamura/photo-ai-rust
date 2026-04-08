/// C ABI exports for photo-engine.
///
/// All functions follow the same contract:
///   - Input:  null-terminated UTF-8 JSON string (caller owns)
///   - Output: written into caller-provided buffer `out_buf` of size `out_len`
///   - Returns: number of bytes written on success (excluding null terminator),
///              negative error code on invalid input/panic,
///              -(required_size as i32) if out_buf is too small

use std::ffi::CStr;
use std::os::raw::c_char;
use std::panic;

use crate::exif;

#[cfg(feature = "excel")]
use crate::excel;

#[cfg(feature = "image")]
use crate::image_proc;

#[cfg(feature = "pdf")]
use crate::pdf;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Convert a raw C string pointer to a Rust `&str`.
///
/// Returns `Err` with an error JSON string if the pointer is null or the
/// bytes are not valid UTF-8.
unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> Result<&'a str, String> {
    if ptr.is_null() {
        return Err("{\"ok\":false,\"error\":\"null input pointer\"}".into());
    }
    CStr::from_ptr(ptr)
        .to_str()
        .map_err(|e| format!("{{\"ok\":false,\"error\":\"invalid UTF-8: {e}\"}}"))
}

/// Write `result` bytes into `out_buf` (capacity `out_len`).
///
/// Returns the number of bytes written (excluding the null terminator) on
/// success, or `-(required as i32)` when the buffer is too small.
/// Returns -1 on a null `out_buf` pointer.
unsafe fn write_to_buf(result: String, out_buf: *mut c_char, out_len: usize) -> i32 {
    if out_buf.is_null() {
        return -1;
    }
    let bytes = result.as_bytes();
    let required = bytes.len() + 1; // +1 for null terminator
    if required > out_len {
        return -(required as i32);
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf as *mut u8, bytes.len());
    *out_buf.add(bytes.len()) = 0; // null terminate
    bytes.len() as i32
}

/// Wrap a fallible closure so that panics are caught and returned as a JSON
/// error string, then written into the caller buffer.
unsafe fn catch_to_buf<F>(f: F, out_buf: *mut c_char, out_len: usize) -> i32
where
    F: FnOnce() -> String + panic::UnwindSafe,
{
    let json = match panic::catch_unwind(f) {
        Ok(s) => s,
        Err(_) => "{\"ok\":false,\"error\":\"internal panic in photo-engine\"}".into(),
    };
    write_to_buf(json, out_buf, out_len)
}

// ── exports ──────────────────────────────────────────────────────────────────

/// Generate a PDF.
///
/// `req_json` must be a JSON object matching `pdf::PdfConfig`.
/// On success, writes a JSON object matching `EngineResponse<pdf::PdfResult>`
/// into `out_buf` and returns the byte count written.
#[cfg(feature = "pdf")]
#[no_mangle]
pub unsafe extern "C" fn photo_engine_generate_pdf(
    req_json: *const c_char,
    out_buf: *mut c_char,
    out_len: usize,
) -> i32 {
    let input = match cstr_to_str(req_json) {
        Ok(s) => s.to_owned(),
        Err(e) => return write_to_buf(e, out_buf, out_len),
    };
    catch_to_buf(
        move || match serde_json::from_str::<pdf::PdfConfig>(&input) {
            Ok(cfg) => pdf::generate_pdf(cfg),
            Err(e) => format!("{{\"ok\":false,\"error\":\"invalid config: {e}\"}}"),
        },
        out_buf,
        out_len,
    )
}

/// Generate an Excel workbook.
///
/// `req_json` must be a JSON object matching `excel::ExcelConfig`.
/// On success, writes a JSON object matching `EngineResponse<excel::ExcelResult>`
/// into `out_buf` and returns the byte count written.
#[cfg(feature = "excel")]
#[no_mangle]
pub unsafe extern "C" fn photo_engine_generate_excel(
    req_json: *const c_char,
    out_buf: *mut c_char,
    out_len: usize,
) -> i32 {
    let input = match cstr_to_str(req_json) {
        Ok(s) => s.to_owned(),
        Err(e) => return write_to_buf(e, out_buf, out_len),
    };
    catch_to_buf(
        move || match serde_json::from_str::<excel::ExcelConfig>(&input) {
            Ok(cfg) => excel::generate_excel(cfg),
            Err(e) => format!("{{\"ok\":false,\"error\":\"invalid config: {e}\"}}"),
        },
        out_buf,
        out_len,
    )
}

/// Process images (resize, thumbnail, contact sheet).
///
/// `req_json` must be a JSON object matching `image_proc::ImageConfig`.
/// On success, writes a JSON object matching `EngineResponse<image_proc::ImageResult>`
/// into `out_buf` and returns the byte count written.
#[cfg(feature = "image")]
#[no_mangle]
pub unsafe extern "C" fn photo_engine_process_image(
    req_json: *const c_char,
    out_buf: *mut c_char,
    out_len: usize,
) -> i32 {
    let input = match cstr_to_str(req_json) {
        Ok(s) => s.to_owned(),
        Err(e) => return write_to_buf(e, out_buf, out_len),
    };
    catch_to_buf(
        move || match serde_json::from_str::<image_proc::ImageConfig>(&input) {
            Ok(cfg) => image_proc::process_image(cfg),
            Err(e) => format!("{{\"ok\":false,\"error\":\"invalid config: {e}\"}}"),
        },
        out_buf,
        out_len,
    )
}

/// Extract EXIF metadata from an image file.
///
/// `req_json` must be a JSON object matching `exif::ExifConfig`.
/// On success, writes a JSON object matching `EngineResponse<exif::ExifResult>`
/// into `out_buf` and returns the byte count written.
#[no_mangle]
pub unsafe extern "C" fn photo_engine_extract_exif(
    req_json: *const c_char,
    out_buf: *mut c_char,
    out_len: usize,
) -> i32 {
    let input = match cstr_to_str(req_json) {
        Ok(s) => s.to_owned(),
        Err(e) => return write_to_buf(e, out_buf, out_len),
    };
    catch_to_buf(
        move || match serde_json::from_str::<exif::ExifConfig>(&input) {
            Ok(cfg) => exif::extract_exif(cfg),
            Err(e) => format!("{{\"ok\":false,\"error\":\"invalid config: {e}\"}}"),
        },
        out_buf,
        out_len,
    )
}
