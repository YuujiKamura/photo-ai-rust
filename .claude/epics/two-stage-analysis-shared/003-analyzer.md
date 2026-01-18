---
name: analyzer-module
description: common/src/analyzer.rs新規作成
status: open
assignee: null
created: 2026-01-18T12:00:00Z
updated: 2026-01-18T12:00:00Z
depends_on: []
blocks: [004]
---

# Issue 003: analyzer.rs作成

## 担当

**空き - 並列実行可能**

## 概要

`common/src/analyzer.rs` を新規作成し、解析ロジックを共通化する。

## 移動元

`src/analyzer/claude_cli.rs` から以下を移動:

```rust
pub fn detect_work_types(raw_data: &[RawImageData]) -> Vec<String>;
pub fn merge_results(raw: &[RawImageData], step2: &[Step2Result], images: &[ImageInfo]) -> Vec<AnalysisResult>;
```

## 実装内容

1. `detect_work_types()` - Step1結果から工種を自動判定
2. `merge_results()` - Step1+Step2結果をマージ
3. CLI/WASM両方で使えるよう抽象化
4. テストを追加

## インターフェース

```rust
use crate::types::{RawImageData, Step2Result, AnalysisResult};

/// Step1結果から工種を自動判定
pub fn detect_work_types(raw_data: &[RawImageData]) -> Vec<String>;

/// 画像メタデータ（CLI/WASM共通）
pub struct ImageMeta {
    pub file_name: String,
    pub file_path: String,  // WASMでは空文字
    pub date: String,       // WASMでは空文字
}

/// Step1+Step2結果をマージ
pub fn merge_results(
    raw_data: &[RawImageData],
    step2_results: &[Step2Result],
    images: &[ImageMeta],
) -> Vec<AnalysisResult>;
```

## 工種判定キーワード

| 工種 | キーワード |
|-----|-----------|
| 舗装工 | 温度, 転圧, 舗設, 敷均し, 乳剤, 路盤, アスファルト, フィニッシャー, ローラー |
| 区画線工 | 区画線, ライン, 白線 |
| 構造物撤去工 | 取壊し, 撤去, 解体 |
| 道路土工 | 掘削, 路床, バックホウ |
| 排水構造物工 | 側溝, 集水, 人孔, マンホール |
| 人孔改良工 | 人孔改良, マンホール蓋 |

## 完了条件

- [ ] analyzer.rs作成
- [ ] detect_work_types関数
- [ ] ImageMeta構造体
- [ ] merge_results関数
- [ ] 単体テスト
