use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UsageMode {
    #[default]
    TimeBasedQuota,
}

/// AI経路設定。Gemini CLI (time-based quota) 固定のため空。
///
/// API後方互換のため残置。全フィールドは削除済み。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CarrierConfig;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupCore {
    pub role: String,
    pub machine_type: String,
    pub machine_id: String,
    #[serde(default)]
    pub has_board: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detected_text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupRecord {
    #[serde(flatten)]
    pub core: GroupCore,
    pub group: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_at: Option<i64>,
}

pub type GroupRecords = HashMap<String, GroupRecord>;
