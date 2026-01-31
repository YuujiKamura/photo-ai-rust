//! Photo AI Common Library
//!
//! CLIとWeb(WASM)で共有される型とユーティリティ
//!
//! ## 解析結果
//! - Step1（画像認識）: RawImageData
//! - 最終出力: AnalysisResult

pub mod types;
pub mod layout;
pub mod alias;
pub mod error;
pub mod hierarchy;
pub mod parser;
pub mod analyzer;
pub mod prompts;
pub mod step2;
#[cfg(feature = "excel")]
pub mod export;

pub use types::{AnalysisResult, RawImageData};
pub use layout::{PdfLayout, ExcelLayout};
pub use alias::{AliasConfig, apply_aliases};
pub use error::{Error, Result};
pub use hierarchy::{HierarchyMaster, HierarchyRow};
pub use parser::{extract_json, parse_step1_response, parse_single_step_response};
pub use analyzer::detect_work_types;
pub use prompts::{PHOTO_CATEGORIES, build_step1_prompt, build_single_step_prompt};
pub use step2::{Step2Result, build_step2_prompt, parse_step2_response, merge_results, ImageMeta};
