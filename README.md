# photo-ai-rust

工事写真AI解析・写真台帳生成ツール（Rust版CLI）

## 関連プロジェクト

**メインリポジトリ**: [GASPhotoAIManager](https://github.com/YuujiKamura/GASPhotoAIManager)

このリポジトリはGASPhotoAIManagerのRust CLI版です。

## 機能

- Claude CLI連携による写真解析
- PDF出力（日本語フォント対応）
- Excel出力
- 工種マスターマッチング

## 使用方法

```bash
# 写真解析
cargo run -- analyze <folder> -o result.json

# PDF/Excel出力
cargo run -- export result.json --format pdf
cargo run -- export result.json --format excel

# 一括実行
cargo run -- run <folder> --format both
```

## レイアウト仕様

`src/export/layout.rs` で定義。GASPhotoAIManager の `utils/layoutConfig.ts` と同一。

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

### フォントサイズ (pt)
| 要素 | サイズ |
|------|--------|
| 全要素 | 12pt（統一） |

### フィールド定義
| key | label | row_span |
|-----|-------|----------|
| date | 日時 | 1 |
| photoCategory | 区分 | 1 |
| workType | 工種 | 1 |
| variety | 種別 | 1 |
| detail | 細別 | 1 |
| station | 測点 | 1 |
| remarks | 備考 | 2 |
| measurements | 測定値 | 3 |
