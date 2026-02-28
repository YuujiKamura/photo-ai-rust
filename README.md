# photo-ai-rust

工事写真AI解析・写真台帳生成CLI（Rust）

## 概要

建設工事の写真をGemini AIで自動解析し、工種階層マスタと照合して写真台帳（PDF/Excel）を生成する。

## 前提条件

- Rust 1.70+
- [photo-tagger](https://github.com/YuujiKamura/photo-tagger) — Gemini AIで写真グループ分け・OCR（必須）
- [Gemini CLI](https://github.com/google-gemini/gemini-cli) — photo-taggerのデフォルトバックエンド（`npm install -g @google/gemini-cli`、Googleアカウント認証）

### AIバックエンド

photo-taggerは内部で [cli-ai-analyzer](https://github.com/YuujiKamura/cli-ai-analyzer) を使用。2つのモードがある:

| モード | 認証 | 費用 | 備考 |
|--------|------|------|------|
| Gemini CLI（デフォルト） | Googleアカウント OAuth | 無料枠 | `gemini auth` で認証 |
| Gemini REST API | `GEMINI_API_KEY` 環境変数 | 従量課金 | cli-ai-analyzer側で `--pay-per-use` 指定 |

現状、photo-taggerはGemini CLIモード固定。REST APIモードを使うにはphoto-tagger側の改修が必要。

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

## ユースケース別レシピ

### 1. 舗装工の施工写真を写真帳にしたい

```bash
# 基本: フォルダ指定 → PDF
photo-ai-rust run ./0213舗装 -m master/by_work_type/舗装工.csv

# 測点が全部同じ場合
photo-ai-rust run ./0213舗装 -m master/by_work_type/舗装工.csv -s "No.5+10.0"

# PDF品質を上げたい（印刷用）
photo-ai-rust run ./0213舗装 -m master/by_work_type/舗装工.csv --pdf-quality high
```

### 2. 区画線工・撤去工など舗装以外

マスタを変えるだけ。使い方は同じ。

```bash
photo-ai-rust run ./区画線 -m master/by_work_type/区画線工.csv
photo-ai-rust run ./撤去 -m master/by_work_type/構造物撤去工.csv
```

区画線工で線種リストがある場合:
```bash
photo-ai-rust run ./区画線 -m master/by_work_type/区画線工.csv --line-types line_types.json
```

### 3. 使用機械・安全管理など写真種類が特定

```bash
# 使用機械写真だけ
photo-ai-rust run ./photos -m master/by_work_type/舗装工.csv -t 使用機械

# 安全管理写真
photo-ai-rust run ./安全 -m master/by_work_type/舗装工.csv -t 安全管理写真
```

### 4. サブフォルダに日付ごとに写真がある

```bash
# 再帰スキャン: 0211/, 0212/, 0213/ をまとめて解析
photo-ai-rust run ./写真 -m master/by_work_type/舗装工.csv -r
```

### 5. 解析結果を手修正してからPDF再生成

```bash
# Step 1: 解析（JSONが出る）
photo-ai-rust analyze ./photos -m master/by_work_type/舗装工.csv -o result.json

# Step 2: result.jsonをエディタで手修正（工種・測点・備考など）

# Step 3: 修正済みJSONからPDF/Excel生成
photo-ai-rust export result.json --format pdf
photo-ai-rust export result.json --format both
```

### 6. 測点・計測値を後から一括設定

```bash
# 全エントリに測点を一括適用
photo-ai-rust normalize result.json -S "No.9"

# 出来形管理写真に車線と備考を設定
photo-ai-rust normalize result.json --lane left --dekigata-remarks "切削基準高"

# 変更前にプレビュー（ドライラン）
photo-ai-rust normalize result.json -S "No.9" --dry-run
```

### 7. 前回解析済みのフォルダを再出力（AIコスト節約）

```bash
# --use-cache: photo-taggerをスキップし、前回のphoto-groups.jsonを再利用
photo-ai-rust run ./photos -m master/by_work_type/舗装工.csv --use-cache
```

### 8. 着手前・竣工写真のペアリング

```bash
# 着手前PDF + 竣工写真フォルダ → ペアリング → フォルダ作成 → PDF
photo-ai-rust pair-completion \
  --before ./着手前写真帳.pdf \
  --after ./竣工写真/ \
  --project-name "南千反畑舗装" \
  --build

# ペアリングJSONだけ出す（手修正してからPDF化したい場合）
photo-ai-rust pair-completion --before ./着手前.pdf --after ./竣工/ -o pairing.json

# 手修正済みJSONからPDF生成
photo-ai-rust pair-pdf --json pairing_manual.json --project-name "南千反畑舗装" ./竣工写真/
```

### 9. 解析精度を検証したい

```bash
# GTファイル（手動正解）と比較
photo-ai-rust evaluate result.json --gt gt.json

# 特定フィールドだけ評価
photo-ai-rust evaluate result.json --gt gt.json --fields remarks,station

# CI用JSON出力
photo-ai-rust evaluate result.json --gt gt.json --json
```

### 10. エイリアスプリセット（ラベル変換）

```bash
# 舗装工向けラベル変換（例: 種別→打換え工種 等）
photo-ai-rust export result.json --preset pavement

# 区画線工向け
photo-ai-rust export result.json --preset marking

# カスタムエイリアスファイル
photo-ai-rust export result.json --alias my_alias.json
```

## コマンドリファレンス

### 主要オプション（run / analyze 共通）

| オプション | 説明 | デフォルト |
|-----------|------|-----------|
| `-m, --master` | 工種マスタCSV | - |
| `-f, --format` | 出力形式 (pdf/excel/xml/both) | pdf |
| `-w, --work-type` | 工種を限定 | - |
| `-t, --photo-type` | 写真種類を限定 | - |
| `--variety` | 種別を限定 | - |
| `-s, --station` | 測点一括指定 | - |
| `--pdf-quality` | PDF品質 (high/medium/low) | medium |
| `--use-cache` | 前回のphoto-groups.jsonを再利用 | off |
| `-r, --recursive` | サブフォルダ再帰スキャン | off |
| `--include-all` | 「非使用」フォルダも含める | off |
| `--line-types` | 区画線の線種リストJSON | - |
| `--folder-rules` | フォルダルールJSON | - |

### export固有オプション

| オプション | 説明 | デフォルト |
|-----------|------|-----------|
| `-p, --photos-per-page` | 1ページあたり写真数 (2/3) | 3 |
| `--preset` | エイリアスプリセット (pavement/marking/general) | - |
| `--alias` | カスタムエイリアスJSON | - |

### normalize固有オプション

| オプション | 説明 | デフォルト |
|-----------|------|-----------|
| `-S, --station` | 測点一括指定 | - |
| `--lane` | 車線 (left/right) | - |
| `--dekigata-remarks` | 出来形備考テキスト | - |
| `--dry-run` | 変更プレビュー | off |
| `--line-types` | 線種リストJSON | - |

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
