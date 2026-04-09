use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageMode {
    PayPerUse,
    Resident,
    TimeBasedQuota,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AiProvider {
    #[default]
    Auto,
    Gemini,
    Claude,
    Codex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BillingMode {
    #[default]
    Auto,
    Subscription,
    PayPerUse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransportMode {
    #[default]
    Auto,
    AgentApi,
    ResidentAgent,
    DirectCli,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CarrierConfig {
    pub provider: AiProvider,
    pub billing: BillingMode,
    pub transport: TransportMode,
}

impl CarrierConfig {
    pub fn effective_usage_mode(self) -> UsageMode {
        if self.billing == BillingMode::PayPerUse {
            UsageMode::PayPerUse
        } else if self.transport == TransportMode::ResidentAgent {
            UsageMode::Resident
        } else {
            UsageMode::TimeBasedQuota
        }
    }
}

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
