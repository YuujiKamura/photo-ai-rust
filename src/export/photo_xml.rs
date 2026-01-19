//! PHOTO.XML 生成（CLI版）
//!
//! GASPhotoAIManager の XML 生成ロジックを移植

use crate::analyzer::AnalysisResult;
use crate::error::Result;
use std::path::{Path, PathBuf};

const DTD_CONTENT: &str = r#"<!ELEMENT 工事写真情報 (電子納品要領基準, 作成日, 写真情報)>
<!ELEMENT 電子納品要領基準 (#PCDATA)>
<!ELEMENT 作成日 (#PCDATA)>
<!ELEMENT 写真情報 (写真*)>
<!ELEMENT 写真 (整理番号, 工種, 種別, 細別, 撮影箇所, 写真タイトル, 写真説明, 写真ファイル名)>
<!ELEMENT 整理番号 (#PCDATA)>
<!ELEMENT 工種 (#PCDATA)>
<!ELEMENT 種別 (#PCDATA)>
<!ELEMENT 細別 (#PCDATA)>
<!ELEMENT 撮影箇所 (#PCDATA)>
<!ELEMENT 写真タイトル (#PCDATA)>
<!ELEMENT 写真説明 (#PCDATA)>
<!ELEMENT 写真ファイル名 (#PCDATA)>"#;

fn escape_xml(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '\'' => out.push_str("&apos;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

fn build_xml(results: &[AnalysisResult]) -> String {
    let date_str = chrono::Utc::now().to_rfc3339();

    let mut xml = String::new();
    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push('\n');
    xml.push_str(r#"<!DOCTYPE 工事写真情報 SYSTEM "PHOTO.DTD">"#);
    xml.push('\n');
    xml.push_str("<工事写真情報>\n");
    xml.push_str("  <電子納品要領基準>案/2010</電子納品要領基準>\n");
    xml.push_str(&format!("  <作成日>{}</作成日>\n", date_str));
    xml.push_str("  <写真情報>\n");

    for (index, result) in results.iter().enumerate() {
        let sort_num = index + 1;
        xml.push_str("    <写真>\n");
        xml.push_str(&format!("      <整理番号>{}</整理番号>\n", sort_num));
        xml.push_str(&format!("      <工種>{}</工種>\n", escape_xml(&result.work_type)));
        xml.push_str(&format!("      <種別>{}</種別>\n", escape_xml(&result.variety)));
        xml.push_str(&format!("      <細別>{}</細別>\n", escape_xml(&result.subphase)));
        xml.push_str(&format!("      <撮影箇所>{}</撮影箇所>\n", escape_xml(&result.station)));
        xml.push_str(&format!("      <写真タイトル>{}</写真タイトル>\n", escape_xml(&result.remarks)));
        xml.push_str(&format!("      <写真説明>{}</写真説明>\n", escape_xml(&result.description)));
        xml.push_str(&format!("      <写真ファイル名>{}</写真ファイル名>\n", escape_xml(&result.file_name)));
        xml.push_str("    </写真>\n");
    }

    xml.push_str("  </写真情報>\n");
    xml.push_str("</工事写真情報>");
    xml
}

fn photo_xml_paths(output: &Path) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let is_xml = output
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("xml"))
        .unwrap_or(false);

    if is_xml {
        let parent = output.parent().unwrap_or_else(|| Path::new("."));
        let dtd_path = parent.join("PHOTO.DTD");
        let pic_dir = parent.join("PIC");
        return Ok((output.to_path_buf(), dtd_path, pic_dir));
    }

    let base_dir = if output.is_dir() || output.extension().is_none() {
        output.to_path_buf()
    } else {
        output.parent().unwrap_or_else(|| Path::new(".")).to_path_buf()
    };

    let photo_dir = base_dir.join("PHOTO");
    std::fs::create_dir_all(&photo_dir)?;

    let xml_path = photo_dir.join("PHOTO.XML");
    let dtd_path = photo_dir.join("PHOTO.DTD");
    let pic_dir = photo_dir.join("PIC");
    Ok((xml_path, dtd_path, pic_dir))
}

pub fn generate_photo_xml(results: &[AnalysisResult], output: &Path) -> Result<PathBuf> {
    let (xml_path, dtd_path, pic_dir) = photo_xml_paths(output)?;
    let xml = build_xml(results);

    std::fs::write(&xml_path, xml)?;
    std::fs::write(&dtd_path, DTD_CONTENT)?;
    copy_images_to_pic(results, &pic_dir)?;

    Ok(xml_path)
}

fn copy_images_to_pic(results: &[AnalysisResult], pic_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(pic_dir)?;

    for result in results {
        if result.file_path.is_empty() {
            continue;
        }
        let source = Path::new(&result.file_path);
        if !source.exists() {
            eprintln!("PHOTO.XML: image not found: {}", source.display());
            continue;
        }
        let file_name = if result.file_name.is_empty() {
            source
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("image.jpg")
                .to_string()
        } else {
            result.file_name.clone()
        };
        let dest = pic_dir.join(file_name);
        if let Err(err) = std::fs::copy(source, &dest) {
            eprintln!(
                "PHOTO.XML: failed to copy image {}: {}",
                source.display(),
                err
            );
        }
    }

    Ok(())
}
