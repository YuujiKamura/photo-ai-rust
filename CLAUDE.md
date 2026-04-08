# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## コマンド

```bash
# ビルド前に LLVM Linker (lld-link) 用の PATH を設定
$env:PATH = "C:\LLVM\bin;" + $env:PATH;

# ビルド
cargo build --release
```
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
- `photo-tagger` (別リポ) - Gemini AIで写真グループ分け

### 工種マスタ
```
master/
└── construction_hierarchy.csv  # 工種マスタ（費目,写真区分,工種,種別,細別,備考,検索パターン）
```

## エージェントの社会性とアイデンティティ（鉄則）

- AIエージェントが作成する**あらゆるアウトプティブ（GitHub Issue、追記、PR、Zenn等への投稿ドラフト、外部メッセージ等）**において、必ず**自身のモデル名（詳しく判らない場合はGemini,Claude,Codexなどキャリア名のみで良い）**を自ら名乗ること。

## 固有名詞禁止（PUBLICリポ）

- 企業名、個人名、特定の工事名称をコード・テスト・ドキュメントに含めるな（.gitignore済みのキャッシュ・ローカルデータは除く）
- テストデータには「テスト工事」「サンプル業者」等のダミーを使え
- コミット前に `grep -rE '(株式会社|有限会社|建設|組)' --include='*.go' --include='*.rs'` で確認
- エージェントに委譲する場合もこのルールは適用される

## 絶対パス禁止

- `C:\Users\`、`/home/`、`/Users/`、`%USERPROFILE%`、ドライブレター付き絶対パスをコード・設定・ドキュメントに書くな
- 動的解決を使え: Rust=`dirs::home_dir()`, Go=`os.UserHomeDir()`, 相対パス, 環境変数
- CLAUDE.md内でも絶対パスでローカルディレクトリを参照するな
- エージェントに作業を委譲する場合もこのルールは適用される
- `/tmp/` のようなOS依存パスも避けろ。テンポラリは `std::env::temp_dir()` / `os.TempDir()` で解決

## 設計原則

- **PDF/Excelが最終成果物** - JSONは中間ファイル
- **photo-tagger必須** - AI fallbackなし、結果空ならエラー終了
- **photo-groups.jsonは消さない** - インクリメンタル、既存結果を保持
- **フォルダ名がカテゴリ** - フォルダ名→マスタ備考で照合
- **PRは作成しない** - masterブランチに直接プッシュ
- **エラー型は各モジュールで定義** - error.rsは合成ハブ（#[from]で集約）
- **PhotoDataトレイト** - get_field_value/get_label_for_fieldでPDF/Excel共通化
- **CSVパースはcsv crate+serde** - CsvRow DTOで外部形式と内部型を分離
