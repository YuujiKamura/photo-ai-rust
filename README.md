# photo-ai

工事写真AI解析・写真台帳生成ツール。

今後のメイン入口は Go フロントエンドの `photo-ai.exe` です。  
解析・PDF・Excel の実処理は Rust 製 engine バイナリに分離したまま使います。

## 構成

```text
photo-ai-go/cmd/photo-ai        # メインCLI（Go）
src/bin/tag_engine.rs           # タグ付け engine（Rust）
photo-engine/src/bin/photo-analysis-engine.rs
src/bin/pdf_engine.rs
src/bin/excel_engine.rs
```

実行時の流れ:

```text
photo-ai.exe
  -> photo-tag-engine.exe
  -> photo-analysis-engine.exe
  -> photo-pdf-engine.exe / photo-excel-engine.exe
```

## ビルド

メインのビルドは Go 側から行う。

```bash
cd photo-ai-go
make all
```

これで次の成果物を作る。

- `photo-ai.exe`
- `target/release/photo-tag-engine.exe`
- `target/release/photo-analysis-engine.exe`
- `target/release/photo-pdf-engine.exe`
- `target/release/photo-excel-engine.exe`

Go フロントだけを再ビルドする場合:

```bash
cd photo-ai-go
make build
```

## 前提条件

- Go 1.25+
- Rust toolchain
- AgentAPI サーバー
- 常駐CLIエージェント
  - Gemini / Claude / Codex のいずれか

## 使い方

詳しい手順は [docs/QUICKSTART.md](./docs/QUICKSTART.md) を見る。

### 解析

```bash
photo-ai analyze ./0213舗装 -m master/by_work_type/舗装工.csv
```

主なオプション:

- `-m, --master`: 工種マスタCSV
- `-w, --work-type`: 工種スコープ
- `-t, --photo-type`: 写真区分スコープ
- `--variety`: 種別スコープ
- `-s, --station`: 測点一括指定
- `-r, --recursive`: サブフォルダ再帰
- `--use-cache`: 既存グループ結果を再利用
- `--pay-per-use`: 従量課金モード

### PDF生成

```bash
photo-ai export pdf ./0213舗装/result.json -o ./0213舗装/工事写真帳.pdf
```

### Excel生成

```bash
photo-ai export excel ./0213舗装/result.json -o ./0213舗装/工事写真帳.xlsx
```

### ペアリング

```bash
photo-ai pair -i ./result.json -o ./paired.json
```

## 出力JSON

`analyze` の最終 `result.json` には、解析結果に加えて次のメタデータを残す。

- `analysisTimestamp`
- `analysisProvider`
- `analysisBilling`
- `analysisTransport`
- `analysisCommit`
- `analysisMasterSelection`
- `analysisMasterPath`
- `analysisScopeWorkType`
- `analysisScopePhotoType`
- `analysisScopeVariety`

これで「いつ」「どのバイナリ」「どの大枠指定」で解析したかを後追いできる。

## 工種マスタ

```text
master/by_work_type/
├── 舗装工.csv
├── 区画線工.csv
├── 構造物撤去工.csv
└── ...
```

## 補足

- Rust CLI の `photo-ai-rust` は開発用・比較用として残っている
- ユーザー向けの主系CLIは `photo-ai.exe`
- CI / リリースも Go フロントエンド前提で組んでいる
- GitHub Pages の静的入口は `web/pages` を正本にしている
