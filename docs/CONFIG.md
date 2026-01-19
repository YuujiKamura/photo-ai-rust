# 設定仕様

## 設定ファイル

デフォルト保存先:

- Linux/macOS: `~/.config/photo-ai/config.json`
- Windows: `%USERPROFILE%\\.config\\photo-ai\\config.json`

環境変数 `ANTHROPIC_API_KEY` が設定されている場合は、設定ファイルより優先されます。

### config.json スキーマ

```json
{
  "api_key": "string or null",
  "model": "claude-sonnet-4-20250514",
  "max_image_size": 1568,
  "default_batch_size": 5,
  "timeout_seconds": 120
}
```

### フィールド説明

- `api_key`: Claude API Key（未設定時は `null`）
- `model`: Claudeモデル名
- `max_image_size`: 画像の最大ピクセル幅（リサイズ上限）
- `default_batch_size`: 解析のバッチ枚数
- `timeout_seconds`: API呼び出しタイムアウト

## 工種マスタ CSV

`HierarchyMaster::from_csv` が読み込むCSVの列は以下の順番です。
先頭行はヘッダーとしてスキップされます。

```
写真区分,写真種別,工種,種別,作業段階,備考,検索パターン
```

### 例

```
写真区分,写真種別,工種,種別,作業段階,備考,検索パターン
"直接工事費","施工状況写真","舗装工","舗装打換え工","表層工","舗設状況",""
"直接工事費","品質管理写真","舗装工","舗装打換え工","表層工","アスファルト混合物温度測定","温度管理|到着温度|敷均し温度"
```

## エイリアス JSON

エイリアスは `photo_category / work_type / variety / subphase` の4つをサポートします。
値は文字列の完全一致 or 部分一致（最長マッチ）で置換されます。

### 例

```json
{
  "photo_category": {
    "品質": "品質管理写真",
    "出来形": "出来形管理写真"
  },
  "work_type": {
    "舗装": "舗装工"
  },
  "variety": {
    "打換え": "舗装打換え工"
  },
  "subphase": {
    "表層": "表層工"
  }
}
```

### プリセット

`pavement / marking / general` を `--preset` で指定できます。
