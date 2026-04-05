/// Shared types for JSON serialization across FFI boundaries.
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct EngineResponse<T: Serialize> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> EngineResponse<T> {
    pub fn success(data: T) -> Self {
        EngineResponse { ok: true, data: Some(data), error: None }
    }
    pub fn failure(msg: impl Into<String>) -> Self {
        EngineResponse { ok: false, data: None, error: Some(msg.into()) }
    }
}

pub fn to_json_string<T: Serialize>(resp: &EngineResponse<T>) -> String {
    serde_json::to_string(resp)
        .unwrap_or_else(|e| format!("{{\"ok\":false,\"error\":\"serialization failed: {e}\"}}"))
}
