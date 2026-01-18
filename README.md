# photo-ai-rust

工事写真AI解析・写真台帳生成ツール（Rust実装）

## Web版

**[Web版を使う (GitHub Pages)](https://yuujikamura.github.io/photo-ai-rust/)**

ブラウザから直接写真をアップロードして解析・PDF出力できます。

## 概要

建設工事の写真を自動解析し、工種・種別・細別を分類して写真台帳（PDF/Excel）を生成するツールです。

### 主な機能

- **AI写真解析**: Claude CLI連携による画像認識・OCR
- **2段階解析**: Step1（画像認識）→ Step2（工種マスタ照合）
- **工種自動判定**: 写真内容から舗装工・区画線工等を自動識別
- **PDF出力**: A4写真台帳（日本語フォント対応）
- **Excel出力**: データ一覧形式
- **キャッシュ機能**: 解析結果をローカルキャッシュ

## インストール

```bash
# ビルド
cargo build --release

# インストール（オプション）
cargo install --path .
```

### 前提条件

- Rust 1.70+
- [Claude CLI](https://github.com/anthropics/claude-code) がインストール・認証済み

## 使用方法

### 基本コマンド

```bash
# 写真解析（JSON出力）
photo-ai-rust analyze <folder> -o result.json

# PDF/Excel出力
photo-ai-rust export result.json --format pdf
photo-ai-rust export result.json --format excel

# 一括実行（解析 → 出力）
photo-ai-rust run <folder> --format both
```

### 2段階解析（工種マスタ使用）

```bash
# 工種階層マスタを指定して解析
photo-ai-rust analyze <folder> --master master/hierarchy.csv -o result.json

# 一括実行
photo-ai-rust run <folder> --master master/hierarchy.csv --format pdf
```

2段階解析の流れ:
1. **Step1**: 画像からOCR・数値・シーン説明を抽出
2. **工種判定**: Step1結果から舗装工・区画線工等を自動識別
3. **Step2**: 絞り込んだマスタと照合して工種・種別・細別を決定

### オプション

```bash
# 解析オプション
--batch-size <N>    # バッチサイズ（デフォルト: 5）
--master <CSV>      # 工種階層マスタCSV
--use-cache         # キャッシュを使用
-v, --verbose       # 詳細出力

# 出力オプション
--format <FORMAT>   # pdf, excel, both
--photos-per-page   # 1ページあたりの写真数（2 or 3）
--title <TITLE>     # 台帳タイトル
--pdf-quality       # PDF品質（high, medium, low）
--preset <NAME>     # エイリアスプリセット（pavement等）
```

### キャッシュ管理

```bash
# キャッシュ情報表示
photo-ai-rust cache --folder <folder>

# キャッシュ削除
photo-ai-rust cache --clear --folder <folder>
```

## プロジェクト構造

```
photo-ai-rust/
├── src/                    # CLI本体
│   ├── analyzer/           # AI解析（Claude CLI連携）
│   ├── export/             # PDF/Excel出力
│   ├── matcher/            # 工種マッチング
│   └── scanner/            # 画像スキャン
├── common/                 # 共有ライブラリ
│   └── src/
│       ├── hierarchy.rs    # 工種階層マスタ
│       ├── alias.rs        # エイリアス変換
│       ├── layout.rs       # レイアウト定義
│       └── types.rs        # 共通型定義
├── web/                    # Web版（HTML/JS）
├── web-wasm/               # Web版（Rust/WASM）予定
└── master/                 # マスタデータ
```

## レイアウト仕様

### ページ設定
| 項目 | 値 |
|------|-----|
| 用紙サイズ | A4 (210mm x 297mm) |
| 余白 | 10mm |
| 写真間ギャップ | 10mm |

### 写真・情報パネル比率
| 項目 | 比率 | 幅 (mm) |
|------|------|---------|
| 写真エリア | 65% | 123.5mm |
| 情報パネル | 35% | 66.5mm |

### 写真高さ
| レイアウト | 高さ (mm) | アスペクト比 |
|-----------|----------|-------------|
| 3枚/ページ | 85.67mm | 1.44:1 (横長) |
| 2枚/ページ | 128.5mm | 0.96:1 (ほぼ正方形) |

## 関連プロジェクト

- [GASPhotoAIManager](https://github.com/YuujiKamura/GASPhotoAIManager) - Google Apps Script版（メインリポジトリ）

## ライセンス

MIT
