//! Photo AI Common Library
//!
//! CLIとWeb(WASM)で共有される型とユーティリティ

pub mod types;
pub mod layout;
pub mod alias;
pub mod error;

pub use types::AnalysisResult;
pub use layout::{PdfLayout, ExcelLayout};
pub use alias::{AliasConfig, apply_aliases};
pub use error::{Error, Result};
