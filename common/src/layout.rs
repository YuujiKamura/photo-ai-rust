//! レイアウト設定モジュール
//!
//! このファイルは後方互換性のためのラッパーです。
//! 実際の定義は layout_generated.rs にあり、
//! shared/layout-config/layout-config.json から自動生成されます。
//!
//! 定数を変更する場合は GASPhotoAIManager/shared/layout-config/layout-config.json を編集し、
//! GASPhotoAIManager で `npm run generate:all && npm run sync:to-rust` を実行してください。

// 生成ファイルをインクルード (モジュールファイルとして)
#[path = "layout_generated.rs"]
mod layout_generated;

// すべての公開アイテムを再エクスポート
pub use layout_generated::*;
