//! Photo AI Common Library
//!
//! CLIとWeb(WASM)で共有される型とユーティリティ
//!
//! ## 2段階解析
//! - Step1（画像認識）: RawImageData
//! - Step2（マスタ照合）: Step2Result
//! - 最終出力: AnalysisResult

pub mod types;
pub mod layout;
pub mod alias;
pub mod error;
pub mod hierarchy;
pub mod parser;
pub mod analyzer;
pub mod prompts;
#[cfg(feature = "excel")]
pub mod export;

pub use types::{AnalysisResult, RawImageData, Step2Result};
pub use layout::{PdfLayout, ExcelLayout};
pub use alias::{AliasConfig, apply_aliases};
pub use error::{Error, Result};
pub use hierarchy::{HierarchyMaster, HierarchyRow};
pub use parser::{extract_json, parse_step1_response, parse_step2_response, parse_single_step_response};
pub use analyzer::{ImageMeta, detect_work_types, merge_results};
pub use prompts::{PHOTO_CATEGORIES, build_step1_prompt, build_step2_prompt, build_single_step_prompt};
