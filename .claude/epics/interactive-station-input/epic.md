---
name: interactive-station-input
status: backlog
created: 2026-01-17T23:59:34Z
progress: 0%
prd: .claude/prds/photo-ai-rust.md
github: [Will be updated when synced to GitHub]
---

# Epic: interactive-station-input

## Overview

AI解析後、測点情報が不明な写真について対話形式で測点を指定できる機能を追加する。現在のワークフロー（解析→Excel出力→手動修正）を改善し、解析直後に測点を確定させることで作業効率を向上させる。

## Architecture Decisions

| 決定事項 | 選択 | 理由 |
|---------|------|------|
| 対話方式 | CLI対話（stdin/stdout） | 既存CLIツールとの整合性、Rustで実装容易 |
| データ保持 | JSON中間ファイル | 既存フローを活かす、中断・再開が容易 |
| 測点候補 | マスタから取得 | 入力補完で効率化 |

## Technical Approach

### CLI対話コンポーネント
- `dialoguer` crateを使用した対話UI
- 写真プレビュー表示（ファイルパス表示 or 外部ビューア起動）
- 測点候補のオートコンプリート
- スキップ・一括適用オプション

### データフロー
```
analyze → JSON(測点なし) → interactive_station → JSON(測点あり) → export
```

### 既存コードへの影響
- `src/cli.rs`: 新コマンド `station` 追加
- `src/analyzer/types.rs`: 変更なし（既存AnalysisResult使用）
- `src/main.rs`: stationコマンドハンドラ追加

## Implementation Strategy

### フェーズ1: 基本対話機能
- 測点が空の写真を抽出
- 1枚ずつ対話で測点入力
- 結果をJSONに保存

### フェーズ2: 効率化
- 測点候補の自動提示
- 「前と同じ」オプション
- 一括スキップ

## Task Breakdown Preview

- [ ] Task 1: dialoguer依存追加とCLI対話基盤
- [ ] Task 2: stationサブコマンド実装
- [ ] Task 3: 測点候補リスト機能
- [ ] Task 4: 一括操作オプション（skip-all, same-as-prev）
- [ ] Task 5: テスト追加

## Dependencies

- `dialoguer` crate（CLI対話UI）
- 既存の`AnalysisResult`構造体
- マスタファイル（測点リスト取得用、オプション）

## Success Criteria (Technical)

| 指標 | 目標 |
|------|------|
| 対話完了率 | 100%（全写真に測点設定可能） |
| 操作性 | 1写真あたり5秒以内で入力可能 |
| 中断耐性 | Ctrl+Cで中断しても途中結果保存 |

## Estimated Effort

- 実装: 3-4時間
- テスト: 1時間
- 合計: 約5時間
