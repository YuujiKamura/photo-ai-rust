# クイックスタート

## 前提条件

- **Go**: 1.25 以上
- **Rust**: 1.70 以上
- **AgentAPI**: 解析 engine から接続できること
- **常駐CLIエージェント**: Gemini / Claude / Codex のいずれか
- **photo-ai**: Goフロント + Rust engine をビルド
  ```bash
  cd photo-ai-go
  make all
  ```
- **(オプション) dropbox-fetch**: Dropbox経由で現場写真を取得する場合
  ```bash
  cargo install --path ../dropbox-fetch
  dropbox-fetch auth   # 初回のみ
  ```

## ワークフロー1: ローカル写真 → 解析 → PDF

写真フォルダを指定して、解析してからPDFを出力する。

```bash
# Step 1: 解析
photo-ai analyze <写真フォルダ> -m master/by_work_type/舗装工.csv

# Step 2: PDF出力
photo-ai export pdf <写真フォルダ>/result.json
```

マスタ未指定時は対話確認に入る。マスタを直接指定する場合:

```bash
photo-ai analyze <写真フォルダ> -m master/by_work_type/舗装工.csv
```

## ワークフロー2: 解析 → 手動調整 → PDF/Excel

解析と出力を分けて、途中で `result.json` を手修正する。

```bash
# Step 1: 解析してJSONを出力
photo-ai analyze <写真フォルダ> -o result.json

# Step 2: result.json を必要に応じて手修正

# Step 3: PDF + Excel を出力
photo-ai export pdf result.json
photo-ai export excel result.json
```

## ワークフロー3: Dropbox取得 → 解析 → PDF

現場スマホで撮った写真をDropbox経由で取得し、そのまま解析する。

```bash
# Step 1: Dropboxから新着写真を取得
dropbox-fetch new "[PLACE_B]" -o ./photos

# Step 2: 取得した写真を解析
photo-ai analyze ./photos -m master/by_work_type/舗装工.csv

# Step 3: PDF出力
photo-ai export pdf ./photos/result.json
```

## よく使うオプション

| オプション | 説明 |
|-----------|------|
| `-m, --master <CSV>` | 工種マスタCSV（省略時は対話選択） |
| `--use-cache` | 前回の解析結果を再利用 |
| `-r, --recursive` | サブフォルダも再帰的にスキャン |
| `--quality high/medium/low` | PDF画像品質（`export pdf`、デフォルト: medium） |
| `-s, --station <TEXT>` | 測点を一括指定 |
| `--pay-per-use` | 従量課金モード |

## トラブルシューティング

- **`gemini` が見つからない**
  ```bash
  npm install -g @google/gemini-cli && gemini auth
  ```

- **マスタが見つからない**
  `--master` でCSVパスを直接指定する。マスタは `master/by_work_type/` に工種別で格納されている。

- **Dropbox認証切れ**
  ```bash
  dropbox-fetch auth
  ```

- **PDFの日本語が文字化け**
  Windows: `C:\Windows\Fonts` に游明朝 (`YuMincho.ttc`) またはメイリオ (`meiryo.ttc`) があるか確認。
