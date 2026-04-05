#ifndef PHOTO_ENGINE_H
#define PHOTO_ENGINE_H

#include <stddef.h>
#include <stdint.h>

/*
 * photo-engine C ABI header
 *
 * All functions accept a null-terminated UTF-8 JSON string (req_json) and
 * write their null-terminated UTF-8 JSON response into caller-provided buffer
 * out_buf of size out_len.
 *
 * Return value:
 *   > 0  : number of bytes written to out_buf (excluding null terminator)
 *   < 0  : -(required buffer size) when out_buf is too small; retry with
 *           a buffer of at least abs(return_value) bytes
 *   -1   : null pointer passed for out_buf
 *
 * Response format (success):  {"ok":true,"data":{...}}
 * Response format (failure):  {"ok":false,"error":"..."}
 *
 * CGo usage example:
 *
 *   #cgo LDFLAGS: -L. -lphoto_engine
 *   #include "photo-engine.h"
 *   import "C"
 *   import "unsafe"
 *
 *   func GeneratePDF(jsonConfig string) string {
 *       cs := C.CString(jsonConfig)
 *       defer C.free(unsafe.Pointer(cs))
 *       buf := make([]byte, 4096)
 *       n := C.photo_engine_generate_pdf(cs,
 *               (*C.char)(unsafe.Pointer(&buf[0])), C.size_t(len(buf)))
 *       if n < 0 {
 *           buf = make([]byte, int(-n))
 *           n = C.photo_engine_generate_pdf(cs,
 *                   (*C.char)(unsafe.Pointer(&buf[0])), C.size_t(len(buf)))
 *       }
 *       return string(buf[:n])
 *   }
 */

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Generate a PDF from the given JSON configuration.
 *
 * Input JSON (PdfConfig):
 *   output_path  string    destination file path
 *   photo_paths  string[]  images to embed
 *   title        string?   optional cover title
 *
 * Output JSON data (PdfResult):
 *   output_path  string    path to generated PDF
 */
int32_t photo_engine_generate_pdf(const char *req_json, char *out_buf, size_t out_len);

/*
 * Generate an Excel workbook from the given JSON configuration.
 *
 * Input JSON (ExcelConfig):
 *   output_path  string      destination file path
 *   rows         string[][]  cell data rows
 *   sheet_name   string?     optional sheet name
 *
 * Output JSON data (ExcelResult):
 *   output_path  string      path to generated Excel file
 */
int32_t photo_engine_generate_excel(const char *req_json, char *out_buf, size_t out_len);

/*
 * Process images (resize / thumbnail / contact sheet).
 *
 * Input JSON (ImageConfig):
 *   input_paths  string[]   source images
 *   output_path  string     destination file path
 *   operation    object     one of:
 *     {"resize":{"width":W,"height":H}}
 *     {"contact_sheet":{"columns":C,"thumb_width":W,"thumb_height":H}}
 *     {"thumbnail":{"max_size":N}}
 *
 * Output JSON data (ImageResult):
 *   output_path  string     path to processed image
 *   width        integer    output width in pixels
 *   height       integer    output height in pixels
 */
int32_t photo_engine_process_image(const char *req_json, char *out_buf, size_t out_len);

/*
 * Extract EXIF metadata from an image file.
 *
 * Input JSON (ExifConfig):
 *   image_path   string   path to image
 *   raw          bool?    if true, return all raw tags
 *
 * Output JSON data (ExifResult):
 *   fields       object   EXIF key-value pairs
 *   gps          object?  {latitude, longitude, altitude?}
 *   datetime     string?  capture time as ISO 8601
 */
int32_t photo_engine_extract_exif(const char *req_json, char *out_buf, size_t out_len);

#ifdef __cplusplus
}
#endif

#endif /* PHOTO_ENGINE_H */
