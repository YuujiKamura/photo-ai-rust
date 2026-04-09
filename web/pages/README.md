# GitHub Pages source

GitHub Pages の静的公開物はこの `web/pages` を正本にする。

## 位置づけ

- `web/`: ローカルで動く実働 Web UI
- `web/pages`: GitHub Pages に載せる静的プレビュー

`web/` 本体は `/api/*` とローカル terminal / websocket に依存するため、そのまま GitHub Pages には載せない。
Pages ではここにある静的入口だけを配信する。

## 更新方針

- ローカル Web UI の構成や見た目が大きく変わったら、この静的ページも追随させる
- 実働ロジックの説明は `README.md` と `docs/QUICKSTART.md` を優先する
