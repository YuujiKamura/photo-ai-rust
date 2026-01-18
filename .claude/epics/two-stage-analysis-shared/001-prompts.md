---
name: prompts-module
description: common/src/prompts.rs新規作成
status: in-progress
assignee: Agent-A (現在のエージェント)
created: 2026-01-18T12:00:00Z
updated: 2026-01-18T12:00:00Z
depends_on: []
blocks: [004]
---

# Issue 001: prompts.rs作成

## 担当

**Agent-A（現在のエージェント）が担当**

## 概要

`common/src/prompts.rs` を新規作成し、プロンプト生成ロジックを共通化する。

## 移動元

`src/analyzer/claude_cli.rs` から以下を移動:

```rust
const PHOTO_CATEGORIES: &[&str] = &[...];
fn build_step1_prompt(images: &[ImageInfo]) -> String;
fn build_step2_prompt(raw_data: &[RawImageData], master: &HierarchyMaster) -> String;
```

## 実装内容

1. `PHOTO_CATEGORIES` 定数をpublic exportとして定義
2. `build_step1_prompt()` - 画像メタデータからStep1プロンプト生成
3. `build_step2_prompt()` - RawImageData+マスタからStep2プロンプト生成
4. テストを追加

## インターフェース

```rust
pub const PHOTO_CATEGORIES: &[&str];

/// Step1プロンプト生成（画像リスト用）
/// CLI: ファイル名+日付のリスト
/// WASM: ファイル名のみ
pub fn build_step1_prompt(file_names: &[(&str, Option<&str>)]) -> String;

/// Step2プロンプト生成（マスタ照合用）
pub fn build_step2_prompt(raw_data: &[RawImageData], master: &HierarchyMaster) -> String;
```

## 完了条件

- [ ] prompts.rs作成
- [ ] PHOTO_CATEGORIES定数
- [ ] build_step1_prompt関数
- [ ] build_step2_prompt関数
- [ ] 単体テスト
