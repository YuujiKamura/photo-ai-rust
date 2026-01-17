# photo-ai-rust 開発プラン

## 概要
工事写真を自動解析してPDF/Excel写真台帳を生成するRust CLIツール

## アーキテクチャ

```
写真フォルダ
    ↓
[1] 画像スキャン (walkdir + image)
    ↓
[2] Claude CLI呼び出し (std::process::Command)
    ↓
[3] 工種マスタ照合 (serde_json)
    ↓
[4] PDF/Excel出力 (printpdf / rust_xlsxwriter)
```

## 重要: Claude CLI方式

**HTTP APIは使わない。Claude CLIを呼び出す。**

理由:
- 認証管理が不要（CLIが管理）
- セッション管理が簡単
- TypeScript版と同じアプローチ

### 呼び出し方式

```rust
use std::process::Command;

// 単発呼び出し
let output = Command::new("claude")
    .args(["-p", &prompt, "--output-format", "text"])
    .output()?;

// 永続プロセス（Phase 2で検討）
let mut child = Command::new("claude")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()?;
```

## フェーズ

### Phase 1: 基盤構築
- [ ] プロジェクト構造
- [ ] CLI引数パース (clap)
- [ ] 設定ファイル読み込み
- [ ] エラーハンドリング (anyhow/thiserror)

### Phase 2: 画像処理
- [ ] フォルダスキャン (walkdir)
- [ ] 画像ファイル一覧取得
- [ ] EXIF日時取得 (kamadak-exif)
- [ ] 画像をtemp-imagesにコピー

### Phase 3: Claude CLI連携
- [ ] Command::new("claude")で呼び出し
- [ ] プロンプト構築（画像パス含む）
- [ ] JSONレスポンスパース
- [ ] バッチ処理
- [ ] 永続プロセス化（オプション）

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
- [ ] Claude CLI永続プロセス化

## 依存クレート

```toml
[dependencies]
# CLI
clap = { version = "4", features = ["derive"] }

# 画像
image = "0.25"
kamadak-exif = "0.5"

# ファイル
walkdir = "2"

# 非同期（CLI出力待ち用）
tokio = { version = "1", features = ["full", "process"] }

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
dirs = "5"
```

**削除したクレート:**
- reqwest（HTTP不要）
- base64（CLI経由なので不要）

## コマンド例

```bash
# 解析してJSON出力
photo-ai analyze ./photos -o result.json

# PDF生成
photo-ai export result.json --format pdf -o 写真台帳.pdf

# 一括処理
photo-ai run ./photos -o 写真台帳.pdf
```

## TypeScript版からの改善点

1. **Claude CLI呼び出しの改善** → Rust のCommand APIで安定化
2. **パス処理の改善** → Rustの PathBuf で正規化
3. **遅延処理の徹底** → 必要時のみファイル操作
4. **型安全性** → Rustの型システムで保証
5. **エラーハンドリング** → Result型で明示的に

## ディレクトリ構造

```
photo-ai-rust/
├── Cargo.toml
├── src/
│   ├── main.rs           # エントリポイント
│   ├── cli.rs            # CLI引数定義
│   ├── config.rs         # 設定
│   ├── error.rs          # エラー型
│   ├── scanner/          # 画像スキャン
│   │   ├── mod.rs
│   │   └── exif.rs
│   ├── analyzer/         # Claude CLI呼び出し
│   │   ├── mod.rs
│   │   ├── claude_cli.rs # CLI実行
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
