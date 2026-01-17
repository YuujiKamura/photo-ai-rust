# photo-ai-rust

工事写真AI解析・写真台帳生成ツール（Rust版CLI）

## 関連プロジェクト

**メインリポジトリ**: [GASPhotoAIManager](https://github.com/YuujiKamura/GASPhotoAIManager)

このリポジトリはGASPhotoAIManagerのRust CLI版です。Issue管理はメインリポジトリで行います。

## 関連Issue・実装状況

### [#139 プレビュー/PDF/Excelのレイアウトを統一する](https://github.com/YuujiKamura/GASPhotoAIManager/issues/139)
- [x] layoutConfig.ts → layout.rs 移植
- [x] LAYOUT_FIELDS定義（8フィールド）
- [x] 65%/35%比率
- [ ] 2枚/ページレイアウト対応

### [#146 PDF画像埋め込み最適化とレイアウト改善](https://github.com/YuujiKamura/GASPhotoAIManager/issues/146)
- [ ] `--pdf-quality` オプション (high/medium/low)
- [ ] 画像圧縮（現在は非圧縮で埋め込み）
- [x] ヘッダー表示
- [x] layoutConfig準拠の配置

### [#147 PDF/Excel出力の改善](https://github.com/YuujiKamura/GASPhotoAIManager/issues/147)
- [ ] Step1キャッシュ (`--use-cache`)
- [ ] エイリアス対応 (`--preset`, `--alias`)
- [ ] テキスト自動縮小・改行
- [ ] 工種マスタ階層構造

### [#148 レイアウト設定の共通化](https://github.com/YuujiKamura/GASPhotoAIManager/issues/148)
- [x] FIELD_LABELS定義
- [x] PDF/Excelで統一ラベル使用
- [ ] 日時フィールド（EXIF取得未実装）
- [ ] DATE_FORMAT設定

## 機能

| 機能 | TypeScript版 | Rust版 |
|------|-------------|--------|
| Claude CLI解析 | ✅ | ✅ |
| PDF出力 | ✅ | ✅ |
| Excel出力 | ✅ | ✅（データのみ） |
| 画像最適化 | ✅ | ❌ |
| Step1キャッシュ | ✅ | ❌ |
| エイリアス | ✅ | ❌ |
| YOLO前処理 | ✅ | ❌ |
| Webサーバー | ✅ | ❌ |

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

GASPhotoAIManager の `utils/layoutConfig.ts` に準拠:
- A4サイズ (210mm x 297mm)
- 余白: 10mm
- 写真:情報 = 65%:35%
- 3枚/ページ時の行高さ: 85.67mm

### フィールド定義 (LAYOUT_FIELDS)

| フィールド | ラベル | row_span |
|-----------|--------|----------|
| date | 日時 | 1 |
| photoCategory | 区分 | 1 |
| workType | 工種 | 1 |
| variety | 種別 | 1 |
| detail | 細別 | 1 |
| station | 測点 | 1 |
| remarks | 備考 | 2 |
| measurements | 測定値 | 3 |
