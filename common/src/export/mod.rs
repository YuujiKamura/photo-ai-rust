//! Export core modules shared across CLI and WASM wrappers.

pub mod pdf_core;

#[cfg(feature = "excel")]
pub mod excel_core;
