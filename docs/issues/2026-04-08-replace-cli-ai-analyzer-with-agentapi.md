# issue draft: cli-ai-analyzer を廃止して AgentAPI ベースへ移行

## 背景

- `photo-ai-rust` の解析系は現在 `deps/cli-ai-analyzer` に強く依存している。
- しかし `deps/cli-ai-analyzer` は薄い CLI wrapper を越えて、`Resident` モード、deckpilot 依存、resume、partial_json などを抱え込み始めており、責務が濁っている。
- 最近の本流は `deckpilot` / `ghostty-web` / Web UI / control plane 側へ移っており、ここで必要なのは「AI 解析ロジック」ではなく「Claude / Codex / Gemini の常駐 CLI エージェントを外から切り替えて叩ける launcher」である。

## 既存 issue との関係

- `#111` path 依存クレート管理の問題
- `#127` ターミナル内 AI エージェントにパイプライン管理を委譲する方向性

今回の issue は、その中でも **`cli-ai-analyzer` を廃止し、launcher を専用層へ置換する** ことに絞る。

## 方針

### 採用候補

- 第一候補: `agentapi`
  - 理由: Claude / Codex / Gemini など既存 CLI エージェントを HTTP API で統一制御できる
  - `photo-ai-rust` 側に必要な「薄い組み込み部品」という要件に最も合う
- 第二候補: `goose`
  - セッション管理まで含んだ完成品ハブとしては強い
  - ただし `photo-ai-rust` に埋め込むにはやや重い

### 非採用

- `deps/cli-ai-analyzer` の機能追加継続
  - launcher とアプリ固有ロジックがさらに混ざるため不採用

## 実装状況（2026-04-08時点）

- `photo-analysis-engine` を追加し、解析層を単独バイナリ化済み
- `src/analyzer/gemini_cli.rs` / `src/pair_ensemble.rs` / `src/analysis.rs` は engine 呼び出しへ切替済み
- CI も `photo-analysis-engine` を build 対象に追加済み
- ただし `ai-code-review` と `desktop-rust` はまだ `cli-ai-analyzer` 依存が残っている

## 置換方針

### Phase 1: root crate の解析入口を直結から外す

- `src/analyzer/gemini_cli.rs`
- `src/pair_ensemble.rs`

ここから `cli-ai-analyzer::analyze()` の直呼びを除去し、`photo-ai-rust` ローカルの launcher 境界へ寄せる。

状態: 完了

### Phase 2: AgentAPI adapter を導入

- `photo-analysis-engine` 内に AgentAPI adapter を追加
- `PHOTO_AI_AGENTAPI_URL` で AgentAPI サーバーを指定
- 画像は `/upload` で渡し、プロンプトは `/message` で送る
- 応答は `/status` + `/messages` で取得

状態: 完了

### Phase 3: 残存依存を除去

- `desktop-rust`
- `deps/ai-code-review`

この2箇所も同じ launcher 境界へ移すか、必要なら別 issue で段階移行する。

状態: 未完了

## 完了条件

- root crate の解析経路が `cli-ai-analyzer` 非依存になる
- 解析・ペアリングが AgentAPI 経由で実行できる
- `cli-ai-analyzer` は `photo-ai-rust` のコア依存から外れる
- 残る依存箇所は別 issue で追跡する

## メモ

- AgentAPI は 2026-04-08 時点で `v0.12.1`
- `/upload`, `/message`, `/messages`, `/status` があるので、常駐 CLI agent を外部 launcher として扱う土台は足りている
