# クイックスタート

## 前提条件

- **Rust**: 1.70 以上
- **Gemini CLI**: photo-tagger（写真グループ分け）が内部で使用
  ```bash
  npm install -g @google/gemini-cli
  gemini auth
  ```
- **photo-ai-rust**: ビルドまたはインストール
  ```bash
  cargo build          # デバッグビルド（推奨。Defenderの誤検知を回避）
  # または
  cargo install --path .
  ```
- **(オプション) dropbox-fetch**: Dropbox経由で現場写真を取得する場合
  ```bash
  cargo install --path ../dropbox-fetch
  dropbox-fetch auth   # 初回のみ
  ```

## ワークフロー1: ローカル写真 → 解析 → PDF

写真フォルダを指定して、解析からPDF出力まで一括実行する。

```bash
# 一括実行（scan → photo-tagger → マスタ照合 → PDF出力）
photo-ai-rust run <写真フォルダ> --format pdf
```

マスタ未指定時は対話式で工種を選択できる。マスタを直接指定する場合:

```bash
photo-ai-rust run <写真フォルダ> -m master/by_work_type/舗装工.csv --format pdf
```

## ワークフロー2: 解析 → 手動調整 → PDF/Excel

解析と出力を分けて、途中で手動調整（測点指定など）を入れる。

```bash
# Step 1: 解析してJSONを出力
photo-ai-rust analyze <写真フォルダ> -o result.json

# Step 2: 測点を一括指定（正規化）
photo-ai-rust normalize result.json -S "No.4 左車線"

# Step 3: PDF + Excel を出力
photo-ai-rust export result.json --format both
```

## ワークフロー3: Dropbox取得 → 解析 → PDF

現場スマホで撮った写真をDropbox経由で取得し、そのまま解析する。

```bash
# Step 1: Dropboxから新着写真を取得
dropbox-fetch new "南千反畑" -o ./photos

# Step 2: 取得した写真を一括解析 → PDF出力
photo-ai-rust run ./photos --format pdf
```

## よく使うオプション

| オプション | 説明 |
|-----------|------|
| `--format pdf/excel/both` | 出力形式 |
| `-m, --master <CSV>` | 工種マスタCSV（省略時は対話選択） |
| `--use-cache` | 前回の解析結果を再利用 |
| `-r, --recursive` | サブフォルダも再帰的にスキャン |
| `--pdf-quality high/medium/low` | PDF画像品質（デフォルト: medium） |
| `-s, --station <TEXT>` | 測点を一括指定 |
| `-v, --verbose` | 詳細ログ出力 |

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

- **releaseビルドがDefenderに削除される**
  デバッグビルド (`cargo build`) を使うか、除外設定を追加する:
  ```powershell
  # 管理者権限で実行（パスは自分の環境に合わせて変更）
  Add-MpExclusion -Path <photo-ai-rustのパス>\target
  ```
