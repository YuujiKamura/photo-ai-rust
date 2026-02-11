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
- `scanner.rs` - 画像スキャン

### 共有ライブラリ（common/src/）
- `types.rs` - AnalysisResult, RawImageData
- `hierarchy.rs` - HierarchyMaster（CSVマスタ読み込み）
- `layout.rs` - PDF/Excelレイアウト定数（mm基準）
- `export/pdf.rs` - PDF生成コア
- `export/excel.rs` - Excel生成コア

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
