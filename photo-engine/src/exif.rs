/// EXIF extraction stub.
/// TODO: Implement from src/scanner/exif.rs in parent crate.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::types::{EngineResponse, to_json_string};

#[derive(Deserialize, Debug)]
pub struct ExifConfig {
    pub image_path: String,
    pub raw: Option<bool>,
}

#[derive(Serialize, Debug)]
pub struct GpsCoords {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
}

#[derive(Serialize, Debug)]
pub struct ExifResult {
    pub fields: HashMap<String, String>,
    pub gps: Option<GpsCoords>,
    pub datetime: Option<String>,
}

pub fn extract_exif(config: ExifConfig) -> String {
    // TODO: Implement using kamadak-exif crate.
    // Reference: src/scanner/exif.rs
    let _ = config;
    let resp: EngineResponse<ExifResult> = EngineResponse::failure("not yet implemented");
    to_json_string(&resp)
}
