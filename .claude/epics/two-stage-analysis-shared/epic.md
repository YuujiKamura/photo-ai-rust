---
name: two-stage-analysis-shared
description: CLI版2段階解析ロジックの共通化・Web版への伝播
status: done
created: 2026-01-18T12:00:00Z
updated: 2026-01-18T12:30:00Z
---

# Epic: CLI版2段階解析ロジックの共通化

## 概要

CLI版で実装した2段階解析（Step1→Step2）、工種自動判定、マスタフィルタリングを
`common/`クレートに共通化し、Web版（WASM）に伝播させる。

## 並列実行計画

```
                        Phase 1a (並列)
                    ┌─ prompts.rs ──┐
types.rs ✓完了 ────>├─ parser.rs  ──├──> lib.rs更新 ──┬─> CLI版リファクタ ──┬──> テスト
                    └─ analyzer.rs ─┘                └─> Web版実装       ──┘
                         ↑並列↑                          ↑並列↑
                                                     Phase 2 (並列)
```

## イシュー一覧

### Phase 1a: 共通モジュール作成 ✅完了

| Issue | 担当 | 状態 | 内容 |
|-------|------|------|------|
| 001 | Agent-A | ✅ 完了 | prompts.rs作成 |
| 002 | Agent-B | ✅ 完了 | parser.rs作成 |
| 003 | Agent-C | ✅ 完了 | analyzer.rs作成 |

### Phase 1b: エクスポート更新 ✅完了

| Issue | 担当 | 状態 | 内容 |
|-------|------|------|------|
| 004 | Agent-A | ✅ 完了 | lib.rs更新 |

### Phase 2: プラットフォーム実装 ✅完了

| Issue | 担当 | 状態 | 内容 |
|-------|------|------|------|
| 005 | Agent-A | ✅ 完了 | CLI版リファクタ (claude_cli.rs) |
| 006 | Agent-B | ✅ 完了 | Web版実装 (gemini.rs + app.rs) |

### Phase 3: 検証 ✅完了

| Issue | 担当 | 状態 | 内容 |
|-------|------|------|------|
| 007 | Agent-A | ✅ 完了 | ビルド・テスト・検証 |

## 完了済み

- [x] types.rs に RawImageData, Step2Result 追加
- [x] prompts.rs 作成 (75テスト通過)
- [x] parser.rs 作成
- [x] analyzer.rs 作成
- [x] lib.rs エクスポート更新
- [x] CLI版リファクタ (共通モジュール使用)
- [x] Web版2段階解析実装
- [x] ビルド・テスト検証 (CLIテスト38件、commonテスト75件)

## 結果

- **common/**: 75テスト通過
- **CLI版**: 38テスト通過、clippy警告なし
- **web-wasm/**: trunk build成功、13テスト通過

## 対象外

| Issue | 状態 | 内容 |
|-------|------|------|
| 008 | wontfix | web/ (HTML+JS版) 凍結 |

- **web/** (HTML+JS版): 凍結。2段階解析未対応のまま放置。
