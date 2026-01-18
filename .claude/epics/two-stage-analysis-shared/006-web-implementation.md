---
name: web-implementation
description: Web版(WASM)2段階解析実装
status: blocked
assignee: null
created: 2026-01-18T12:00:00Z
updated: 2026-01-18T12:00:00Z
depends_on: [004]
blocks: [007]
parallel_with: [005]
---

# Issue 006: Web版2段階解析実装

## 担当

**待機中 - Issue 004完了後に実行可能（Issue 005と並列可能）**

## 依存関係

- 依存: Issue 004 (lib.rs更新)
- 並列: Issue 005 (CLI版リファクタ)
- ブロック: Issue 007 (テスト)

## 概要

`web-wasm/src/api/gemini.rs` を2段階解析に対応させる。

## 変更内容

### gemini.rs

1. 共通モジュールをインポート
2. `analyze_photo_step1()` 追加 - Step1実行
3. `analyze_photo_step2()` 追加 - Step2実行
4. `analyze_photo()` を2段階対応に変更
5. マスタフィルタリング追加

```rust
use photo_ai_common::{
    RawImageData, Step2Result, HierarchyMaster,
    build_step1_prompt, build_step2_prompt,
    parse_step1_response, parse_step2_response,
    detect_work_types, merge_results, ImageMeta,
};

/// Step1: 画像認識
pub async fn analyze_step1(
    api_key: &str,
    images: &[(String, String)],  // (file_name, data_url)
) -> Result<Vec<RawImageData>, JsValue>;

/// Step2: マスタ照合
pub async fn analyze_step2(
    api_key: &str,
    raw_data: &[RawImageData],
    master: &HierarchyMaster,
) -> Result<Vec<Step2Result>, JsValue>;

/// 2段階解析（マスタあり）
pub async fn analyze_with_master(
    api_key: &str,
    images: &[(String, String)],
    master: &HierarchyMaster,
) -> Result<Vec<AnalysisResult>, JsValue>;
```

### app.rs

1. マスタデータのロード追加（埋め込みまたはfetch）
2. 解析フローを2段階に変更
3. 進捗表示を2段階対応に更新

## 完了条件

- [ ] gemini.rs 2段階解析対応
- [ ] マスタデータのWASM対応
- [ ] app.rs 解析フロー更新
- [ ] trunk build成功
