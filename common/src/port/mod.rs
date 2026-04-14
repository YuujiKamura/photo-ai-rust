//! Port traits（ヘキサゴナルアーキテクチャの "port" 層）
//!
//! Application 層が Infrastructure に依存しないよう、I/O を trait で抽象化する。
//! 実装（adapter）は infra/ 側に置く。
//!
//! - `master`: マスタCSVのロード
//! - （将来）`report`: PDF/Excel/XML のレンダリング
//! - （将来）`vision`: AI 画像解析 Oracle

pub mod master;
pub mod report;
pub mod vision;

pub use master::{MasterError, MasterRepository};
pub use report::{ImageData, MockRenderer, OutputFormat, RenderError, ReportRenderer};
pub use vision::{ImageRef, MockCallLog, MockOracle, OracleError, VisionOracle};
