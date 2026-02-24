//! PDF画像抽出モジュール
//!
//! 着手前写真帳PDFからページごとの画像とテキストを抽出する。

use crate::error::{PhotoAiError, Result};
use std::path::{Path, PathBuf};

/// PDFから抽出された1ページ分のデータ
pub struct ExtractedPage {
    pub page_num: u32,
    pub image_path: PathBuf,
    pub station_text: String,
}

/// フォルダ内の画像ファイルを ExtractedPage として返す（PDF代替入力用）
pub fn extract_pages_from_folder(folder: &Path) -> Result<Vec<ExtractedPage>> {
    if !folder.exists() || !folder.is_dir() {
        return Err(PhotoAiError::FolderNotFound(folder.display().to_string()));
    }

    let mut files: Vec<PathBuf> = std::fs::read_dir(folder)
        .map_err(|e| PhotoAiError::PdfExtraction(format!("フォルダ読み込み失敗: {}", e)))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| matches!(ext.to_lowercase().as_str(), "jpg" | "jpeg" | "png"))
                    .unwrap_or(false)
        })
        .collect();

    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    let extracted = files
        .into_iter()
        .enumerate()
        .map(|(i, path)| {
            let station_text = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            ExtractedPage {
                page_num: (i + 1) as u32,
                image_path: path,
                station_text,
            }
        })
        .collect();

    Ok(extracted)
}

/// PDFの各ページから最大の JPEG 画像を抽出し、テキスト情報と共に返す
pub fn extract_images_from_pdf(pdf_path: &Path) -> Result<Vec<ExtractedPage>> {
    if !pdf_path.exists() {
        return Err(PhotoAiError::FileNotFound(pdf_path.display().to_string()));
    }

    let doc = lopdf::Document::load(pdf_path)
        .map_err(|e| PhotoAiError::PdfExtraction(format!("PDF読み込み失敗: {}", e)))?;

    let temp_dir = std::env::temp_dir().join(format!("photo-ai-pair-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| PhotoAiError::PdfExtraction(format!("tempディレクトリ作成失敗: {}", e)))?;

    let pages = doc.get_pages();
    let mut extracted = Vec::new();

    let mut sorted_pages: Vec<_> = pages.into_iter().collect();
    sorted_pages.sort_by_key(|(num, _)| *num);

    for (page_num, page_id) in &sorted_pages {
        if let Some(image_data) = extract_first_image_from_page(&doc, *page_id) {
            let image_path = temp_dir.join(format!("page_{:02}.jpg", page_num));
            if let Err(e) = std::fs::write(&image_path, &image_data) {
                eprintln!("  Warning: page {} 画像保存失敗: {}", page_num, e);
                continue;
            }

            let station_text = extract_page_text(pdf_path, *page_num);

            extracted.push(ExtractedPage {
                page_num: *page_num,
                image_path,
                station_text,
            });
        }
    }

    Ok(extracted)
}

/// ページ内の最初のJPEG画像（DCTDecode）を抽出
fn extract_first_image_from_page(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Option<Vec<u8>> {
    let page_obj = doc.get_object(page_id).ok()?;
    let page_dict = page_obj.as_dict().ok()?;

    let resources = match page_dict.get(b"Resources") {
        Ok(r) => doc.dereference(r).ok().map(|(_, o)| o)?,
        Err(_) => return None,
    };
    let resources_dict = resources.as_dict().ok()?;

    let xobject = match resources_dict.get(b"XObject") {
        Ok(x) => doc.dereference(x).ok().map(|(_, o)| o)?,
        Err(_) => return None,
    };
    let xobject_dict = xobject.as_dict().ok()?;

    let mut best_image: Option<Vec<u8>> = None;
    let mut best_size: usize = 0;

    for (_name, obj_ref) in xobject_dict.iter() {
        let Ok((_, obj)) = doc.dereference(obj_ref) else {
            continue;
        };
        let Ok(stream) = obj.as_stream() else {
            continue;
        };
        let dict = &stream.dict;

        if let Ok(subtype) = dict.get(b"Subtype") {
            let subtype_obj = doc
                .dereference(subtype)
                .map(|(_, o)| o)
                .unwrap_or(subtype);
            if let Ok(name) = subtype_obj.as_name_str() {
                if name != "Image" {
                    continue;
                }
            }
        }

        let Ok(filter) = dict.get(b"Filter") else {
            continue;
        };
        let filter_obj = doc
            .dereference(filter)
            .map(|(_, o)| o)
            .unwrap_or(filter);
        let is_dct = match filter_obj.as_name_str() {
            Ok(name) => name == "DCTDecode",
            Err(_) => {
                if let Ok(arr) = filter_obj.as_array() {
                    arr.iter().any(|f| {
                        doc.dereference(f)
                            .ok()
                            .and_then(|(_, o)| o.as_name_str().ok().map(|n| n == "DCTDecode"))
                            .unwrap_or(false)
                    })
                } else {
                    false
                }
            }
        };

        if is_dct {
            let data = stream.content.clone();
            if data.len() > best_size {
                best_size = data.len();
                best_image = Some(data);
            }
        }
    }

    best_image
}

/// ページテキストを抽出（測点名など）
fn extract_page_text(pdf_path: &Path, page_num: u32) -> String {
    let options = pdf_analysis_embed::ExtractOptions::new().with_pages(vec![page_num]);
    pdf_analysis_embed::extract_text_with_options(pdf_path, &options)
        .unwrap_or_default()
        .trim()
        .to_string()
}
