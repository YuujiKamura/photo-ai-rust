use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ResultItem {
    pub file_name: String,
    pub file_path: String,
    pub date: String,
    pub photo_category: String,
    pub work_type: String,
    pub variety: String,
    pub detail: String,
    pub remarks: String,
    pub station: String,
    pub description: String,
    pub measurements: String,
    pub detected_text: String,
    pub has_board: bool,
    pub reasoning: String,
}

#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub items: Vec<ResultItem>,
    pub original_items: Vec<ResultItem>,
    pub selected_index: Option<usize>,
    pub source_path: Option<std::path::PathBuf>,
    pub dirty: bool,
}
