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
    scan_folder_with_options(folder, false)
}

pub fn scan_folder_recursive(folder: &Path) -> Result<Vec<ImageInfo>> {
    scan_folder_with_options(folder, true)
}

pub fn scan_folder_with_options(folder: &Path, recursive: bool) -> Result<Vec<ImageInfo>> {
    if !folder.exists() {
        return Err(PhotoAiError::FolderNotFound(folder.display().to_string()));
    }

    let mut images = Vec::new();

    let walker = if recursive {
        WalkDir::new(folder)
    } else {
        WalkDir::new(folder).max_depth(1)
    };

    for entry in walker.into_iter().filter_map(|e| e.ok())
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

/// Check if a file extension is a supported image format
#[cfg(test)]
fn is_image_extension(ext: &str) -> bool {
    IMAGE_EXTENSIONS.contains(&ext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;

    /// Generate unique temp directory for each test to avoid parallel test conflicts
    fn unique_temp_dir(test_name: &str) -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("photo-ai-{}-{}", test_name, timestamp))
    }

    /// Cleanup helper that ensures directory is removed even if test panics
    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn test_is_image_extension() {
        assert!(is_image_extension("jpg"));
        assert!(is_image_extension("JPG"));
        assert!(is_image_extension("jpeg"));
        assert!(is_image_extension("png"));
        assert!(!is_image_extension("txt"));
        assert!(!is_image_extension("pdf"));
        assert!(!is_image_extension("gif"));
    }

    #[test]
    fn test_scan_folder_not_found() {
        let result = scan_folder(Path::new("/nonexistent/folder/that/does/not/exist"));
        assert!(result.is_err());
    }

    #[test]
    fn test_scan_folder_empty() {
        let temp_dir = unique_temp_dir("empty");
        let _guard = TempDirGuard(temp_dir.clone());
        fs::create_dir_all(&temp_dir).unwrap();

        let result = scan_folder(&temp_dir).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_scan_folder_with_images() {
        let temp_dir = unique_temp_dir("images");
        let _guard = TempDirGuard(temp_dir.clone());
        fs::create_dir_all(&temp_dir).unwrap();

        // Create dummy image files
        File::create(temp_dir.join("test1.jpg")).unwrap().write_all(b"dummy").unwrap();
        File::create(temp_dir.join("test2.JPG")).unwrap().write_all(b"dummy").unwrap();
        File::create(temp_dir.join("test3.png")).unwrap().write_all(b"dummy").unwrap();
        File::create(temp_dir.join("readme.txt")).unwrap().write_all(b"text").unwrap();

        let result = scan_folder(&temp_dir).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].file_name, "test1.jpg");
        assert_eq!(result[1].file_name, "test2.JPG");
        assert_eq!(result[2].file_name, "test3.png");
    }

    #[test]
    fn test_images_sorted_by_filename() {
        let temp_dir = unique_temp_dir("sort");
        let _guard = TempDirGuard(temp_dir.clone());
        fs::create_dir_all(&temp_dir).unwrap();

        File::create(temp_dir.join("c.jpg")).unwrap();
        File::create(temp_dir.join("a.jpg")).unwrap();
        File::create(temp_dir.join("b.jpg")).unwrap();

        let result = scan_folder(&temp_dir).unwrap();
        assert_eq!(result[0].file_name, "a.jpg");
        assert_eq!(result[1].file_name, "b.jpg");
        assert_eq!(result[2].file_name, "c.jpg");
    }
}
