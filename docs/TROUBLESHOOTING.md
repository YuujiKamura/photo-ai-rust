# トラブルシューティング

## Claude CLI が見つからない

症状:
- `claude` コマンドが見つからない/実行できない
- `Claude CLI failed` が発生する

対処:
- Claude CLI をインストールし、`PATH` に追加
- `claude -h` が動作することを確認

## 日本語フォントが見つからない（PDF出力）

症状:
- PDFで日本語が `?` になる

対処:
- Windows: `C:\\Windows\\Fonts` に `YuMincho.ttc` / `msmincho.ttc` / `meiryo.ttc` があるか確認
- フォントがない場合は、明朝/ゴシック系フォントを追加

## API呼び出し限度に達した

症状:
- APIレスポンスがエラーになる / 429 が返る

対処:
- バッチサイズを下げる (`--batch-size`)
- 再試行まで待機する
- `--use-cache` で再解析を抑制

## よくあるエラー

### JSONパースエラー

原因:
- 解析結果のJSONが崩れている

対処:
- `--verbose` でレスポンスを確認
- 画像枚数を減らして再実行

### ファイルが見つからない

原因:
- 入力パス/出力パスの指定ミス

対処:
- `--input` / `--output` / `--master` のパスを確認
