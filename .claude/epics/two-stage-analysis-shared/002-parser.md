---
name: parser-module
description: common/src/parser.rs新規作成
status: open
assignee: null
created: 2026-01-18T12:00:00Z
updated: 2026-01-18T12:00:00Z
depends_on: []
blocks: [004]
---

# Issue 002: parser.rs作成

## 担当

**空き - 並列実行可能**

## 概要

`common/src/parser.rs` を新規作成し、JSONパースロジックを共通化する。

## 移動元

`src/analyzer/claude_cli.rs` から以下を移動:

```rust
fn extract_json(response: &str) -> Result<&str>;
fn parse_step1_response(response: &str) -> Result<Vec<RawImageData>>;
fn parse_step2_response(response: &str) -> Result<Vec<Step2Result>>;
```

## 実装内容

1. `extract_json()` - レスポンスからJSON部分を抽出
2. `parse_step1_response()` - Step1レスポンスをパース
3. `parse_step2_response()` - Step2レスポンスをパース
4. エラー型は `common::Error` を使用
5. テストを追加

## インターフェース

```rust
use crate::error::Result;
use crate::types::{RawImageData, Step2Result};

/// APIレスポンスからJSON部分を抽出
pub fn extract_json(response: &str) -> Result<&str>;

/// Step1レスポンスをパース
pub fn parse_step1_response(response: &str) -> Result<Vec<RawImageData>>;

/// Step2レスポンスをパース
pub fn parse_step2_response(response: &str) -> Result<Vec<Step2Result>>;
```

## 完了条件

- [ ] parser.rs作成
- [ ] extract_json関数
- [ ] parse_step1_response関数
- [ ] parse_step2_response関数
- [ ] 単体テスト（JSON block, raw JSON, エラーケース）
