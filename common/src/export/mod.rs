//! Export core modules shared across CLI and WASM wrappers.

pub mod pdf;

#[cfg(feature = "excel")]
pub mod excel;
