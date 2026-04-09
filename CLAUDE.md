# CLAUDE.md

This file provides guidance to the AI agent when working with code in this repository.

## コマンド

```bash
# Goフロント + Rust engine をまとめてビルド
cd photo-ai-go && make all

# Go フロントだけ再ビルド
cd photo-ai-go && make build

# テスト
cargo test                      # 全テスト
cargo test normalizer           # モジュール指定
cargo test -p photo-ai-common   # commonクレートのみ

# 推奨フロー: analyze → export
photo-ai analyze <folder> -m master/by_work_type/舗装工.csv
photo-ai export pdf result.json
```

## アーキテクチャ

### ワークスペース構成
```
photo-ai-rust/          # リポジトリルート
├── photo-ai-go/        # メインCLI（Go）
├── common/             # 共有ライブラリ（photo-ai-common）
├── photo-engine/       # 解析 engine
├── web-wasm/           # WASM版（未完成）
└── desktop-rust/       # デスクトップ版（未完成）
```

### 解析フロー
```
photo-ai.exe
  → photo-tag-engine.exe
  → photo-analysis-engine.exe
  → photo-pdf-engine.exe / photo-excel-engine.exe
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

### 工種マスタ
```
master/
├── by_work_type/               # 工種別CSV
└── common.csv                  # 共通エントリ
```

## エージェントの社会性とアイデンティティ（鉄則）

- **CI/CD Fixes:** 実装後のビルドや CI が単純なミス（依存関係の不足、バージョン不一致、軽微なリンターエラー等）で失敗した場合は、ユーザーの確認を待たずに自律的に修正して再プッシュすること。
- AIエージェントが作成する**あらゆるアウトプット（GitHub Issue、追記、PR、外部メッセージ等）**において、必ず**自身の正しい名称**を自ら名乗ること。例示として特定の名称が挙がっていても、それに惑わされてはならない。

## 固有名詞禁止（PUBLICリポ）

- 企業名、個人名、特定の工事名称をコード・テスト・ドキュメントに含めるな（.gitignore済みのキャッシュ・ローカルデータは除く）
- テストデータには「テスト工事」「サンプル業者」等のダミーを使え
- コミット前に `grep -rE '(株式会社|有限会社|建設|組)' --include='*.go' --include='*.rs'` で確認
- エージェントに委譲する場合もこのルールは適用される

## 絶対パス禁止

- ドライブレター付き絶対パス、OS依存パス（C:\\Users\\など）をコード・設定・ドキュメントに書くな
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
- **主系CLIは Go** - Rust CLI `photo-ai-rust` は比較用・開発用
