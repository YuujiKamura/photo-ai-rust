//! ドメイン層
//!
//! 工事写真管理の業務ルールと定数を集約する。
//!
//! - `constants`: 写真区分・工種・種別等の正規名称
//! - `policy`: 写真分類に関する判定ルール（安全管理・温度・機械関連・OCR揺れ正規化）
//!
//! ## 設計方針
//!
//! Strength-Distance-Variability の観点から、
//! 揮発性の高い業務ルールは距離（モジュール間）を超えて散在させない。
//! 新しい業務ルールを追加するときは、まずこのモジュールを拡張すること。

pub mod constants;
pub mod photo_category;
pub mod policy;

pub use constants::*;
pub use photo_category::PhotoCategory;
