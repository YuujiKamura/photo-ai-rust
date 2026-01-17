use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResult {
    pub file_name: String,

    #[serde(default)]
    pub work_type: String,        // 工種

    #[serde(default)]
    pub variety: String,          // 種別

    #[serde(default)]
    pub detail: String,           // 細別

    #[serde(default)]
    pub station: String,          // 測点

    #[serde(default)]
    pub remarks: String,          // 備考

    #[serde(default)]
    pub description: String,      // 写真説明

    #[serde(default)]
    pub has_board: bool,          // 黒板あり

    #[serde(default)]
    pub detected_text: String,    // OCRテキスト

    #[serde(default)]
    pub measurements: String,     // 数値データ

    #[serde(default)]
    pub photo_category: String,   // 写真区分

    #[serde(default)]
    pub reasoning: String,        // 分類理由
}

impl Default for AnalysisResult {
    fn default() -> Self {
        Self {
            file_name: String::new(),
            work_type: String::new(),
            variety: String::new(),
            detail: String::new(),
            station: String::new(),
            remarks: String::new(),
            description: String::new(),
            has_board: false,
            detected_text: String::new(),
            measurements: String::new(),
            photo_category: String::new(),
            reasoning: String::new(),
        }
    }
}
