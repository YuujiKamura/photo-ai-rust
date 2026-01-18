---
name: test-verify
description: ビルド・テスト・検証
status: blocked
assignee: null
created: 2026-01-18T12:00:00Z
updated: 2026-01-18T12:00:00Z
depends_on: [005, 006]
blocks: []
---

# Issue 007: テスト・検証

## 担当

**待機中 - Issue 005, 006完了後に実行**

## 依存関係

- 依存: Issue 005 (CLI版), 006 (Web版)

## 検証内容

### 1. common/のテスト

```bash
cd common && cargo test
```

### 2. CLI版ビルド・テスト

```bash
cargo build --release
cargo test
cargo clippy

# 実行テスト
photo-ai-rust analyze test-photos --master master/construction_hierarchy.csv -o test.json -v
```

### 3. Web版ビルド

```bash
cd web-wasm && trunk build
```

### 4. Web版動作確認

```bash
trunk serve
# ブラウザで写真アップロード → 2段階解析 → PDF出力
```

### 5. CLI版とWeb版の結果比較

同じ画像セットで解析し、結果の一致を確認:
- work_type, variety, detail の一致率
- photo_category の一致率
- detected_text の精度比較

## 完了条件

- [ ] common/ テスト通過
- [ ] CLI版 ビルド成功
- [ ] CLI版 テスト通過
- [ ] Web版 ビルド成功
- [ ] 動作確認完了
