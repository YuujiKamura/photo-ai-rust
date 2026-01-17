/// マスタエントリ（Excelから読み込み）
#[derive(Debug, Clone)]
pub struct MasterEntry {
    pub photo_category: String,
    pub work_type: String,
    pub variety: String,
    pub detail: String,
    pub match_patterns: Vec<String>,
}

/// 照合結果
#[derive(Debug, Clone, Default)]
pub struct MatchResult {
    pub work_type: String,
    pub variety: String,
    pub detail: String,
    pub matched_patterns: Vec<String>,
    pub confidence: f32,
}
