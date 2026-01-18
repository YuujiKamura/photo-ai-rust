---
name: web-js-frozen
description: web/ (HTML+JS版) を凍結
status: closed
assignee: null
created: 2026-01-18T12:30:00Z
updated: 2026-01-18T12:30:00Z
resolution: wontfix
---

# Issue 008: web/ (HTML+JS版) 凍結

## 決定

**web/ ディレクトリは凍結。2段階解析は実装しない。**

## 理由

- JSはメンテナンス対象外
- Rust (web-wasm/) に一本化

## 現状

| 項目 | web/ (JS) | web-wasm/ (Rust) |
|-----|-----------|------------------|
| 解析 | 1段階 | 2段階 ✅ |
| マスタ照合 | なし | あり ✅ |
| 工種自動判定 | なし | あり ✅ |
| 状態 | 凍結 | アクティブ |

## 対応

- `web/FROZEN.md` 作成済み
- 今後の機能追加は `web-wasm/` のみ

## 推奨コマンド

```bash
# web-wasm/ を使用
cd web-wasm && trunk serve
```
