# photo-ai-rust

工事写真AI解析・写真台帳生成ツール（Rust版CLI）

## 関連プロジェクト

**メインリポジトリ**: [GASPhotoAIManager](https://github.com/YuujiKamura/GASPhotoAIManager)

このリポジトリはGASPhotoAIManagerのRust CLI版です。Issue管理はメインリポジトリで行います。

## 関連Issue

- [#139 プレビュー/PDF/Excelのレイアウトを統一する](https://github.com/YuujiKamura/GASPhotoAIManager/issues/139)
- [#146 PDF画像埋め込み最適化とレイアウト改善](https://github.com/YuujiKamura/GASPhotoAIManager/issues/146)
- [#147 PDF/Excel出力の改善](https://github.com/YuujiKamura/GASPhotoAIManager/issues/147)

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

GASPhotoAIManager の `utils/layoutConfig.ts` に準拠:
- A4サイズ (210mm x 297mm)
- 余白: 10mm
- 写真:情報 = 65%:35%
- 3枚/ページ時の行高さ: 85.67mm
