mod exif;

use crate::error::{PhotoAiError, Result};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub path: PathBuf,
    pub file_name: String,
    pub date: Option<String>,
}

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "JPG", "JPEG", "PNG"];

pub fn scan_folder(folder: &Path) -> Result<Vec<ImageInfo>> {
    if !folder.exists() {
        return Err(PhotoAiError::FolderNotFound(folder.display().to_string()));
    }

    let mut images = Vec::new();

    for entry in WalkDir::new(folder)
        .max_depth(1)  // 直下のみ（再帰しない）
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy();
            if IMAGE_EXTENSIONS.iter().any(|&e| e == ext_str) {
                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                let date = exif::extract_date(path).ok();

                images.push(ImageInfo {
                    path: path.to_path_buf(),
                    file_name,
                    date,
                });
            }
        }
    }

    // ファイル名でソート
    images.sort_by(|a, b| a.file_name.cmp(&b.file_name));

    Ok(images)
}
