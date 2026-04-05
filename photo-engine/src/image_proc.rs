/// Image processing stub.
/// TODO: Implement from src/contactsheet.rs in parent crate.
use serde::{Deserialize, Serialize};
use crate::types::{EngineResponse, to_json_string};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ImageOperation {
    Resize { width: u32, height: u32 },
    ContactSheet { columns: u32, thumb_width: u32, thumb_height: u32 },
    Thumbnail { max_size: u32 },
}

#[derive(Deserialize, Debug)]
pub struct ImageConfig {
    pub input_paths: Vec<String>,
    pub output_path: String,
    pub operation: ImageOperation,
}

#[derive(Serialize, Debug)]
pub struct ImageResult {
    pub output_path: String,
    pub width: u32,
    pub height: u32,
}

pub fn process_image(config: ImageConfig) -> String {
    // TODO: Implement using image and imageproc crates.
    // Reference: src/contactsheet.rs
    let _ = config;
    let resp: EngineResponse<ImageResult> = EngineResponse::failure("not yet implemented");
    to_json_string(&resp)
}
