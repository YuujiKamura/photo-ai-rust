# photo-ai-rust

工事写真AI解析・写真台帳生成CLI（Rust）

## 概要

建設工事の写真をGemini AIで自動解析し、工種階層マスタと照合して写真台帳（PDF/Excel）を生成する。

## 前提条件

- Rust 1.70+
- [photo-tagger](https://github.com/YuujiKamura/photo-tagger) — Gemini APIで写真グループ分け・OCR（必須）
- Gemini API key（photo-tagger用）

```bash
photo-ai-rust doctor   # 前提条件を一括チェック
```

## インストール

```bash
cargo build --release
```

## 解析パイプライン

```
写真フォルダ
  → scan（画像収集）
  → photo-tagger（Gemini API：グループ分け・OCR・focus_target抽出）
  → マスタ照合（detected_text + folder_name → 工種階層マッチング）
  → グループ伝播（リーダーのマッチ結果を同一グループに適用）
  → ドメイン補正（工種変換・線種検出）
  → 正規化（測定値統一・測点適用）
  → export（PDF/Excel）
```

詳細は [Issue #104](https://github.com/YuujiKamura/photo-ai-rust/issues/104) を参照。

## 使用方法

### 一括実行（推奨）

```bash
# 解析 → PDF出力
photo-ai-rust run ./photos -m master/by_work_type/舗装工.csv --format pdf

# 解析 → PDF + Excel
photo-ai-rust run ./photos -m master/by_work_type/舗装工.csv --format both

# キャッシュ使用（再解析スキップ）
photo-ai-rust run ./photos -m master/by_work_type/舗装工.csv --use-cache --format pdf
```

### 個別実行

```bash
# 解析のみ（JSON出力）
photo-ai-rust analyze ./photos -m master/by_work_type/舗装工.csv

# 出力のみ（既存JSONから）
photo-ai-rust export result.json --format pdf

# 正規化（グループ単位で計測値統一）
photo-ai-rust normalize result.json -S "No.1"
```

### 主要オプション

| オプション | 説明 | デフォルト |
|-----------|------|-----------|
| `-m, --master` | 工種マスタCSV | - |
| `-f, --format` | 出力形式 (pdf/excel/xml/both) | pdf |
| `-w, --work-type` | 工種指定 | - |
| `-s, --station` | 測点一括指定 | - |
| `--pdf-quality` | PDF品質 (high/medium/low) | medium |
| `--use-cache` | 解析キャッシュ使用 | off |
| `-r, --recursive` | サブフォルダ再帰スキャン | off |
| `--preset` | エイリアスプリセット (pavement/marking/general) | - |

## 工種マスタ

`master/by_work_type/` に工種別CSVを配置:

```
master/by_work_type/
├── 舗装工.csv
├── 区画線工.csv
├── 構造物撤去工.csv
├── 道路土工.csv
└── ...
```

CSV列: 費目, 写真区分, 工種, 種別, 細別, 備考, 検索パターン

## プロジェクト構成

```
photo-ai-rust/
├── src/                    # CLI本体
│   ├── main.rs             # エントリポイント
│   ├── commands.rs         # コマンドハンドラ
│   ├── analysis.rs         # パイプライン制御
│   ├── master_matcher.rs   # マスタ照合
│   ├── normalizer/         # 正規化（測定値統一・測点）
│   ├── export/             # PDF/Excel出力
│   ├── scanner/            # 画像スキャン
│   └── temperature.rs      # 温度管理写真処理
├── common/                 # 共有ライブラリ（photo-ai-common）
│   └── src/
│       ├── types.rs        # AnalysisResult, PhotoDataトレイト
│       ├── hierarchy/      # 工種階層マスタ読み込み
│       ├── export/         # PDF/Excelコア（レイアウト・描画）
│       └── layout.rs       # レイアウト定数・FieldKey
├── master/                 # 工種マスタCSV
├── web-wasm/               # WASM版（未完成・凍結）
└── desktop-rust/           # デスクトップ版（未完成・凍結）
```

## サブコマンド一覧

| コマンド | 用途 |
|---------|------|
| `run` | 解析→出力の一括実行 |
| `analyze` | 写真解析（JSON出力） |
| `export` | JSON→PDF/Excel変換 |
| `normalize` | 正規化（計測値統一・測点適用） |
| `evaluate` | 解析精度評価（GT比較） |
| `pair-completion` | 着手前・竣工写真の自動ペアリング |
| `pair-pdf` | 着手前竣工写真帳PDF生成 |
| `station` | 対話的測点入力 |
| `cache` | キャッシュ管理 |
| `doctor` | 前提条件チェック |
| `config` | 設定表示/編集 |

## ライセンス

MIT
