use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub fn extract_date(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut bufreader = BufReader::new(file);
    let exif_reader = exif::Reader::new();
    let exif = exif_reader.read_from_container(&mut bufreader)?;

    // DateTimeOriginal を探す
    if let Some(field) = exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY) {
        return Ok(field.display_value().to_string());
    }

    // DateTime を探す
    if let Some(field) = exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY) {
        return Ok(field.display_value().to_string());
    }

    Err("No date found in EXIF".into())
}
