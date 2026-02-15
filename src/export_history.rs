use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportHistoryEntry {
    pub timestamp: String,
    pub input: String,
    pub format: String,
    pub output: String,
    pub photos_per_page: u8,
    pub title: String,
    pub pdf_quality: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportHistory {
    pub version: u32,
    pub max_entries: usize,
    pub entries: Vec<ExportHistoryEntry>,
}

impl Default for ExportHistory {
    fn default() -> Self {
        Self {
            version: 1,
            max_entries: 20,
            entries: Vec::new(),
        }
    }
}

pub fn history_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("photo-ai")
        .join("export_history.json")
}

pub fn load() -> ExportHistory {
    let path = history_path();
    if !path.exists() {
        return ExportHistory::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => ExportHistory::default(),
    }
}

pub fn record(entry: ExportHistoryEntry) -> std::io::Result<()> {
    let mut history = load();

    // 同一inputがあれば更新、なければ先頭に追加
    if let Some(pos) = history.entries.iter().position(|e| e.input == entry.input) {
        history.entries.remove(pos);
    }
    history.entries.insert(0, entry);

    // max_entries超えたら末尾削除
    if history.entries.len() > history.max_entries {
        history.entries.truncate(history.max_entries);
    }

    let path = history_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&history)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(&path, json)
}

pub fn list() -> Vec<ExportHistoryEntry> {
    load().entries
}
