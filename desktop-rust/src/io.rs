use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::model::ResultItem;

pub fn load_result_items(path: &Path) -> Result<Vec<ResultItem>> {
    let content = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let items: Vec<ResultItem> = serde_json::from_str(&content)
        .with_context(|| format!("parse {}", path.display()))?;
    Ok(items)
}

pub fn save_sorted_items(path: &Path, items: &[ResultItem]) -> Result<()> {
    let content = serde_json::to_string_pretty(items)?;
    fs::write(path, content).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn default_sorted_path(source: &Path) -> PathBuf {
    let file_name = source
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("result.json");
    let sorted_name = if file_name.to_lowercase().ends_with(".json") {
        file_name.to_string().replace(".json", ".sorted.json")
    } else {
        format!("{file_name}.sorted.json")
    };
    source.with_file_name(sorted_name)
}
