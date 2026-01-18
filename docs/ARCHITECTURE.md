# アーキテクチャ概要

## ワークスペース構成

```
photo-ai-rust/
  common/           共通ライブラリ（型、プロンプト、解析コア、exportコア）
  src/              CLI実装（画像解析、PDF/Excel出力）
  web-wasm/         Web実装（Leptos + WASM、JSブリッジでPDF/Excel出力）
```

## 依存関係（CLI / Web）

```
CLI (src/)
  -> photo-ai-common
      -> prompts / parser / analyzer / hierarchy
      -> export::pdf_core / export::excel_core
  -> printpdf / rust_xlsxwriter / image

Web (web-wasm/)
  -> photo-ai-common (wasm)
      -> prompts / parser / analyzer / hierarchy
      -> export::pdf_core / export::excel_core (レイアウト/フィールド統一)
  -> JS Bridge (pdf-lib / ExcelJS)
```

## 2段階解析のシーケンス

```
User/CLI            Analyzer                 Claude CLI                 Common
   |                   |                         |                        |
   | 画像読み込み        |                         |                        |
   |------------------>| build_step1_prompt()    |                        |
   |                   |--------------prompt---->|                        |
   |                   |<-------------JSON-------| parse_step1_response() |
   |                   |---- RawImageData ----->|                        |
   |                   | detect_work_types()     |                        |
   |                   | build_step2_prompt()    |                        |
   |                   |--------------prompt---->|                        |
   |                   |<-------------JSON-------| parse_step2_response() |
   |                   | merge_results()         |                        |
   |<------------------| AnalysisResult          |                        |
```

## データフロー

```
画像フォルダ
  -> 解析 (Step1 + Step2)
  -> AnalysisResult (JSON)
  -> エイリアス適用 (任意)
  -> Export
       -> PDF (printpdf / pdf-lib)
       -> Excel (rust_xlsxwriter / ExcelJS)
```

## Exportの共通化

```
common/src/export/
  pdf_core.rs     レイアウト・フィールド定義の共通化
  excel_core.rs   セル配置・フォーマットの共通化
```
