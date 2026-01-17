use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

