---
name: lib-export-update
description: common/src/lib.rsエクスポート更新
status: blocked
assignee: null
created: 2026-01-18T12:00:00Z
updated: 2026-01-18T12:00:00Z
depends_on: [001, 002, 003]
blocks: [005, 006]
---

# Issue 004: lib.rs更新

## 担当

**待機中 - Issue 001-003完了後に実行可能**

## 依存関係

- 依存: Issue 001 (prompts.rs), 002 (parser.rs), 003 (analyzer.rs)
- ブロック: Issue 005 (CLI), 006 (Web)

## 概要

`common/src/lib.rs` を更新し、新規モジュールをエクスポートする。

## 変更内容

```rust
// 追加
pub mod prompts;
pub mod parser;
pub mod analyzer;

// エクスポート追加
pub use types::{RawImageData, Step2Result};
pub use prompts::{PHOTO_CATEGORIES, build_step1_prompt, build_step2_prompt};
pub use parser::{extract_json, parse_step1_response, parse_step2_response};
pub use analyzer::{detect_work_types, merge_results, ImageMeta};
```

## 完了条件

- [ ] prompts モジュール追加
- [ ] parser モジュール追加
- [ ] analyzer モジュール追加
- [ ] 型・関数のre-export
- [ ] cargo build成功
