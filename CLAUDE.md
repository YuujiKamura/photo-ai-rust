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

# 解析実行
cargo run --release -- analyze <folder> --work-type 舗装補修工
cargo run --release -- normalize result.json -S "12月26日"
cargo run --release -- export result.json --format pdf
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
写真 → scanner → analyzer(Claude CLI) → normalizer → export(PDF/Excel)
                      ↓
                 HierarchyMaster（工種マスタ照合）
```

### 主要モジュール（CLIクレート: src/）
- `analyzer/claude_cli.rs` - Claude CLI呼び出し、プロンプト生成
- `normalizer/` - 後処理（3枚セット内で黒板アップの計測値に統一）
- `export/pdf.rs`, `export/excel.rs` - PDF/Excel生成
- `matcher/` - 工種マスタとのマッチング

### 共有ライブラリ（common/src/）
- `types.rs` - AnalysisResult, RawImageData
- `prompts.rs` - AIプロンプト生成（build_single_step_prompt）
- `hierarchy.rs` - HierarchyMaster（CSVマスタ読み込み）
- `layout.rs` - PDF/Excelレイアウト定数（mm基準）
- `parser.rs` - AIレスポンスのJSONパース

### 工種マスタ
```
master/
├── construction_hierarchy.csv  # 汎用マスタ
└── by_work_type/
    ├── 舗装補修工.csv          # 工種別マスタ
    └── 区画線工.csv
```

## 設計原則

- **PDF/Excelが最終成果物** - JSONは中間ファイル
- **1ステップ解析優先** - 工種指定時は2ステップ不要
- **黒板アップが正** - 温度管理写真は3枚セット、黒板アップの値に統一
- **PRは作成しない** - masterブランチに直接プッシュ
