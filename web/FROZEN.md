# 凍結

このディレクトリは**凍結**されています。

## 理由

- HTML+JavaScript版は1段階解析のまま
- 2段階解析は `web-wasm/` (Leptos + WASM) で実装済み
- JS版のメンテナンスは行わない

## 推奨

`web-wasm/` を使用してください。

```bash
cd web-wasm && trunk serve
```
