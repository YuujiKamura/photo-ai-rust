//! Photo AI Common Library
//!
//! CLIとWeb(WASM)で共有される型とユーティリティ
//!
//! ## 解析結果
//! - Step1（画像認識）: RawImageData
//! - 最終出力: AnalysisResult

pub mod domain;
pub mod port;
pub mod types;
pub mod layout;
pub mod alias;
pub mod error;
pub mod hierarchy;
pub mod parser;
pub mod analyzer;
pub mod work_type_defs;
pub mod prompt_format;
pub mod prompts;
pub mod step2;
pub mod evaluate;
#[cfg(any(feature = "excel", feature = "pdf"))]
pub mod export;

pub use types::{AnalysisResult, RawImageData, PhotoData, WorkTypeDefinition, LineTypeEntry, LineTypesConfig, normalize_exif_datetime};
pub use layout::{PdfLayout, ExcelLayout, FieldKey};
pub use alias::{AliasConfig, apply_aliases, longest_match_transform};
pub use error::{Error, Result, HierarchyError, AnalyzerError};
#[cfg(any(feature = "excel", feature = "pdf"))]
pub use error::ExportError;
pub use hierarchy::{HierarchyMaster, HierarchyRow};
pub use parser::{extract_json, parse_step1_response, parse_single_step_response};
pub use analyzer::{detect_work_types, detect_work_types_with};
pub use work_type_defs::DEFAULT_WORK_TYPE_DEFINITIONS;
pub use prompt_format::{hierarchy_to_json, hierarchy_to_compact_text, hierarchy_to_chain_records};
pub use prompts::{PHOTO_CATEGORIES, build_step1_prompt, build_prompt_for_category};
pub use step2::{Step2Result, build_step2_prompt, parse_step2_response, merge_results, ImageMeta};
