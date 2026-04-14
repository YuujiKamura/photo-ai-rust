//! VisionOracle port の具体 adapter 群
//!
//! 各 adapter は `photo_ai_common::port::vision::VisionOracle` trait を実装し、
//! 実際の外部 AI CLI（Gemini/Claude/Codex）呼び出しを port 契約の裏に隠す。

pub mod gemini;

pub use gemini::GeminiCliOracle;
