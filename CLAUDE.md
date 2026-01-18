# photo-ai-rust プロジェクト設定

## 許可されたコマンド

```
Write(*)
Edit(*)
Read(*)
Bash(cargo:*)
Bash(rustc:*)
Bash(rustup:*)
Bash(git:*)
Bash(gh:*)
Bash(mkdir:*)
Bash(rm:*)
Bash(mv:*)
Bash(cp:*)
Bash(ls:*)
Bash(cat:*)
Bash(head:*)
Bash(tail:*)
Bash(echo:*)
Bash(curl:*)
Bash(wget:*)
Bash(date:*)
Bash(sed:*)
Bash(grep:*)
Bash(awk:*)
Bash(find:*)
Bash(wc:*)
Bash(sort:*)
Bash(uniq:*)
Bash(test:*)
Bash(bash:*)
Bash(for:*)
Bash(which:*)
Bash(python:*)
Bash(codex:*)
```

## 自動化ルール

### コード変更後
1. `cargo build` でビルド確認
2. `cargo clippy` で警告確認
3. エラーがなければコミット・プッシュ

### PRについて
- PRは作成しない
- masterブランチに直接プッシュ

## 設計原則

### 最終出力
**PDF/Excel が最終成果物**
- JSON/CSVは中間ファイル

### 遅延処理
- base64化は必要になるまで遅延
- ファイル読み込みは必要時のみ

### エラーハンドリング
- Result型で明示的に
- ユーザーフレンドリーなメッセージ

## プロジェクト概要
工事写真AI解析・写真台帳生成CLIツール（Rust実装）
