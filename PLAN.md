# photo-ai-rust 開発プラン

## 概要
工事写真を自動解析してPDF/Excel写真台帳を生成するRust CLIツール

## アーキテクチャ

```
写真フォルダ
    ↓
[1] 画像スキャン (walkdir + image)
    ↓
[2] Claude API解析 (reqwest + base64)
    ↓
[3] 工種マスタ照合 (serde_json)
    ↓
[4] PDF/Excel出力 (printpdf / rust_xlsxwriter)
```

## フェーズ

### Phase 1: 基盤構築
- [ ] プロジェクト構造
- [ ] CLI引数パース (clap)
- [ ] 設定ファイル読み込み (config)
- [ ] エラーハンドリング (anyhow/thiserror)

### Phase 2: 画像処理
- [ ] フォルダスキャン (walkdir)
- [ ] 画像読み込み・リサイズ (image)
- [ ] Base64エンコード (base64)
- [ ] EXIF日時取得 (kamadak-exif)

### Phase 3: Claude API連携
- [ ] API呼び出し (reqwest)
- [ ] Vision APIでの画像解析
- [ ] JSONレスポンスパース
- [ ] バッチ処理・リトライ

### Phase 4: マスタ照合
- [ ] 工種マスタJSON読み込み
- [ ] 階層構造マッチング
- [ ] 分類結果のマージ

### Phase 5: 出力生成
- [ ] PDF生成 (printpdf)
  - 写真レイアウト（2枚/3枚）
  - 日本語フォント埋め込み
  - 表形式の工種情報
- [ ] Excel生成 (rust_xlsxwriter)
  - 写真埋め込み
  - セル書式設定

### Phase 6: 最適化
- [ ] 並列処理 (rayon)
- [ ] プログレス表示 (indicatif)
- [ ] キャッシュ機構

## 依存クレート

```toml
[dependencies]
# CLI
clap = { version = "4", features = ["derive"] }
# 画像
image = "0.25"
base64 = "0.22"
kamadak-exif = "0.5"
# ファイル
walkdir = "2"
# HTTP
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
tokio = { version = "1", features = ["full"] }
# JSON
serde = { version = "1", features = ["derive"] }
serde_json = "1"
# PDF
printpdf = "0.7"
# Excel
rust_xlsxwriter = "0.79"
# ユーティリティ
anyhow = "1"
thiserror = "2"
indicatif = "0.17"
rayon = "1"
```

## コマンド例

```bash
# 解析してJSON出力
photo-ai analyze ./photos -o result.json

# PDF生成
photo-ai export result.json --format pdf -o 写真台帳.pdf

# 一括処理
photo-ai run ./photos -o 写真台帳.pdf

# サーバーモード（将来）
photo-ai serve --port 3001
```

## TypeScript版からの移行ポイント

1. **Claude CLI依存を排除** → API直接呼び出し
2. **遅延処理の徹底** → 必要時のみbase64化
3. **型安全性** → Rustの型システムで保証
4. **エラーハンドリング** → Result型で明示的に

## ディレクトリ構造

```
photo-ai-rust/
├── Cargo.toml
├── src/
│   ├── main.rs           # エントリポイント
│   ├── cli.rs            # CLI引数定義
│   ├── config.rs         # 設定
│   ├── scanner/          # 画像スキャン
│   │   ├── mod.rs
│   │   └── exif.rs
│   ├── analyzer/         # Claude API
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   └── types.rs
│   ├── matcher/          # マスタ照合
│   │   └── mod.rs
│   └── export/           # 出力生成
│       ├── mod.rs
│       ├── pdf.rs
│       └── excel.rs
├── master/               # 工種マスタ
│   └── hierarchy.json
└── tests/
```
