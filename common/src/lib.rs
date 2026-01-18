//! Photo AI Common Library
//!
//! CLIとWeb(WASM)で共有される型とユーティリティ

pub mod types;
pub mod layout;
pub mod alias;
pub mod error;
pub mod hierarchy;
pub mod parser;
pub mod analyzer;

pub use types::AnalysisResult;
pub use layout::{PdfLayout, ExcelLayout};
pub use alias::{AliasConfig, apply_aliases};
pub use error::{Error, Result};
pub use hierarchy::{HierarchyMaster, HierarchyRow};
pub use parser::{extract_json, parse_step1_response, parse_step2_response};
pub use analyzer::{ImageMeta, detect_work_types, merge_results};
