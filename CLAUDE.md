# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## コマンド

```bash
# ビルド
cargo build --release

# テスト
cargo test                      # 全テスト
cargo test normalizer           # モジュール指定
cargo test -p photo-ai-common   # commonクレートのみ

# 一括処理（推奨）: scan → tagger → master → export
cargo run --release -- run <folder> --master master/construction_hierarchy.csv

# 個別コマンド
cargo run --release -- analyze <folder>              # scan + tagger + master
cargo run --release -- normalize result.json -S "No.1" # 測点一括適用
cargo run --release -- export result.json --format pdf  # PDF/Excel出力
```

## アーキテクチャ

### ワークスペース構成
```
photo-ai-rust/          # CLI本体（メインクレート）
├── common/             # 共有ライブラリ（photo-ai-common）
├── web-wasm/           # WASM版（未完成）
└── desktop-rust/       # デスクトップ版（未完成）
```

### 解析フロー
```
写真 → scan → photo-tagger（Gemini、必須） → マスタ照合 → 正規化 → export(PDF/Excel)
                    ↓                              ↓
            photo-groups.json              construction_hierarchy.csv
           （インクリメンタル）              （フォルダ名→備考で照合）
```

### 主要モジュール（CLIクレート: src/）
- `commands.rs` - コマンドハンドラ（run, analyze, export等）
- `analysis.rs` - パイプライン（scan→tagger→master→normalize）
- `normalizer/` - 後処理（3枚セット内で黒板アップの計測値に統一）
  - `alias.rs` - プリセット別フィールド変換（舗装/区画線/撤去等）
- `scanner.rs` - 画像スキャン

### 共有ライブラリ（common/src/）
- `types.rs` - AnalysisResult, RawImageData, PhotoDataトレイト, WorkTypeDefinition
- `hierarchy/` - 工種階層マスタ
  - `mod.rs` - HierarchyMaster, HierarchyRow, HierarchyError
  - `csv_parser.rs` - csv crate+serdeによるCSVパース（CsvRow DTO）
- `analyzer.rs` - 工種自動判定（detect_work_types）, AnalyzerError
- `layout.rs` - PDF/Excelレイアウト定数、FieldKey enum、fit_image_centered()
- `prompt_format.rs` - マスタ→AIプロンプト整形（JSON/compact/chain形式）
- `export/` - エクスポート
  - `mod.rs` - Exporterトレイト, ImageData, ExportError
  - `pdf.rs` - PDF生成コア（PdfMetrics, PdfInfoField）
  - `pdf_embed.rs` - PDF Info辞書への解析結果埋め込み（非WASM）
  - `excel.rs` - Excel生成コア
- `error.rs` - Error合成ハブ（ExportError/HierarchyError/AnalyzerErrorをre-export）

### 外部依存
- `photo-tagger` (`C:/Users/yuuji/photo-tagger`) - Gemini AIで写真グループ分け

### 工種マスタ
```
master/
└── construction_hierarchy.csv  # 工種マスタ（費目,写真区分,工種,種別,細別,備考,検索パターン）
```

## 設計原則

- **PDF/Excelが最終成果物** - JSONは中間ファイル
- **photo-tagger必須** - AI fallbackなし、結果空ならエラー終了
- **photo-groups.jsonは消さない** - インクリメンタル、既存結果を保持
- **フォルダ名がカテゴリ** - フォルダ名→マスタ備考で照合
- **PRは作成しない** - masterブランチに直接プッシュ
- **エラー型は各モジュールで定義** - error.rsは合成ハブ（#[from]で集約）
- **PhotoDataトレイト** - get_field_value/get_label_for_fieldでPDF/Excel共通化
- **CSVパースはcsv crate+serde** - CsvRow DTOで外部形式と内部型を分離
