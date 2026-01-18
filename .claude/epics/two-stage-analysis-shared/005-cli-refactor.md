---
name: cli-refactor
description: CLI版リファクタ - claude_cli.rsを共通モジュール使用に変更
status: blocked
assignee: null
created: 2026-01-18T12:00:00Z
updated: 2026-01-18T12:00:00Z
depends_on: [004]
blocks: [007]
parallel_with: [006]
---

# Issue 005: CLI版リファクタ

## 担当

**待機中 - Issue 004完了後に実行可能（Issue 006と並列可能）**

## 依存関係

- 依存: Issue 004 (lib.rs更新)
- 並列: Issue 006 (Web版実装)
- ブロック: Issue 007 (テスト)

## 概要

`src/analyzer/claude_cli.rs` から共通化した部分を削除し、`common` クレートからインポートするように変更。

## 変更内容

### 削除対象（common/に移動済み）

- `RawImageData` 構造体
- `Step2Result` 構造体
- `PHOTO_CATEGORIES` 定数
- `build_step1_prompt()` 関数
- `build_step2_prompt()` 関数
- `extract_json()` 関数
- `parse_step1_response()` 関数
- `parse_step2_response()` 関数
- `detect_work_types()` 関数
- `merge_results()` 関数

### 残す部分（CLI固有）

- `run_claude_cli()` - Claude CLI呼び出し
- `copy_to_temp()` - 画像コピー
- `get_temp_dir()` - 一時ディレクトリ取得
- `analyze_batch_step1()` - Step1実行（run_claude_cli使用）
- `analyze_batch_step2()` - Step2実行（run_claude_cli使用）
- `analyze_batch()` - 後方互換性維持
- `analyze_batch_with_master()` - 2段階解析実行

### インポート追加

```rust
use photo_ai_common::{
    RawImageData, Step2Result,
    PHOTO_CATEGORIES, build_step1_prompt, build_step2_prompt,
    extract_json, parse_step1_response, parse_step2_response,
    detect_work_types, merge_results, ImageMeta,
};
```

## 完了条件

- [ ] 共通部分を削除
- [ ] common::からインポート
- [ ] ImageInfoからImageMetaへの変換追加
- [ ] cargo build成功
- [ ] 既存テスト通過
