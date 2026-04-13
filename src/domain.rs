//! ドメイン定数モジュール（再エクスポート）
//!
//! 実体は `photo_ai_common::domain` にある。
//! 既存の `use crate::domain::*;` を温存するためのシム。
//! 新規コードは直接 `photo_ai_common::domain::{constants, policy}` を参照してよい。

pub use photo_ai_common::domain::constants::*;
pub use photo_ai_common::domain::policy;
