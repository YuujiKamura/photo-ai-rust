# Master Storage Separation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 配布バイナリに埋め込む engine と、ユーザーが更新可能な master データを責務分離し、版ズレと運用不能を防ぐ。

**Architecture:** engine はリリース時に固定された配布物として同一コミットから埋め込み、master はユーザー領域に seed 展開して外部ファイルとして運用する。実行時はユーザー master を優先し、存在しない場合のみ seed/embedded にフォールバックする。master は CSV 互換を維持しつつ JSON マニフェストで schema/version を管理する。

**Tech Stack:** Go (`photo-ai-go`), Rust (`photo-engine`, `common`), embedded FS, JSON manifest, CSV compatibility

---

### Task 1: Master Runtime Contract を定義する

**Files:**
- Create: `docs/ARCHITECTURE-master-runtime.md`
- Modify: `README.md`
- Modify: `docs/TROUBLESHOOTING.md`

**Step 1: Write the failing doc assertions**

定義すべき必須ルール:

- engine は配布物固定、master は外部可変
- master 解決順は `user data dir -> repo/workspace dir -> embedded seed`
- master には `schema_version` と `master_version` を付与
- engine は対応 `schema_version` の範囲を宣言する

**Step 2: Run doc grep to verify nothing already defines this completely**

Run:

```powershell
rg -n "schema_version|master_version|user data dir|embedded seed" README.md docs photo-ai-go photo-engine
```

Expected: 断片的な記述のみで、統一契約は未定義

**Step 3: Write the minimal architecture doc**

以下を明記する:

- 配布物に含まれるもの
- 初回起動時の seed 展開
- ユーザー編集対象
- 互換性チェック
- 破損時の復旧方法

**Step 4: Update README/TROUBLESHOOTING**

追記項目:

- master の保存場所
- バックアップ/復旧
- `--master` 明示指定時の優先順位

**Step 5: Commit**

```bash
git add docs/ARCHITECTURE-master-runtime.md README.md docs/TROUBLESHOOTING.md
git commit -m "docs: define engine and master runtime contract"
```

### Task 2: JSON Manifest を追加して master 版管理を導入する

**Files:**
- Create: `photo-ai-go/internal/master/manifest.json`
- Create: `photo-ai-go/internal/master/manifest.go`
- Create: `common/src/master_manifest.rs`
- Test: `photo-ai-go/internal/master/manifest_test.go`
- Test: `common/src/master_manifest.rs`

**Step 1: Write the failing tests**

Go 側:

- manifest が読める
- `schema_version` と `master_version` が空なら失敗
- CSV 一覧と manifest 一覧が不一致なら失敗

Rust 側:

- manifest JSON を parse できる
- 対応 schema 範囲外なら弾ける

**Step 2: Run tests to verify they fail**

Run:

```bash
cd photo-ai-go && go test ./internal/master
cd .. && cargo test -p photo-ai-rust master_manifest
```

Expected: manifest 未実装で FAIL

**Step 3: Write minimal manifest**

JSON shape:

```json
{
  "schema_version": 1,
  "master_version": "2026-04-11",
  "files": [
    { "name": "舗装工", "file": "by_work_type/舗装工.csv", "sha256": "..." }
  ]
}
```

**Step 4: Implement parser/validator**

必須ロジック:

- embedded master file 群と manifest の整合確認
- Rust engine が受け付ける `schema_version` チェック

**Step 5: Re-run tests**

```bash
cd photo-ai-go && go test ./internal/master
cd .. && cargo test -p photo-ai-rust master_manifest
```

Expected: PASS

**Step 6: Commit**

```bash
git add photo-ai-go/internal/master common/src/master_manifest.rs
git commit -m "feat: add master manifest and schema versioning"
```

### Task 3: User Master Directory 解決を追加する

**Files:**
- Create: `photo-ai-go/internal/master/runtime.go`
- Create: `photo-ai-go/internal/master/runtime_test.go`
- Modify: `photo-ai-go/cmd/photo-ai/serve.go`
- Modify: `photo-ai-go/pkg/engine/engine.go` if needed only for path passing

**Step 1: Write the failing tests**

テストケース:

- user data dir が存在すればそれを使う
- user data dir が無ければ embedded を seed 展開する
- repo 内 `master/by_work_type` があれば dev mode ではそれを使う
- 壊れた manifest は embedded から復旧できる

**Step 2: Run Go tests to verify they fail**

```bash
cd photo-ai-go && go test ./internal/master ./cmd/photo-ai
```

**Step 3: Implement runtime resolver**

API 例:

```go
type MasterSource struct {
    RootDir string
    Manifest Manifest
    Source string // user|workspace|embedded-seed
}

func ResolveMasterSource(repoDir string) (MasterSource, error)
```

保存先候補:

- `%LOCALAPPDATA%/photo-ai/master/`
- 開発時のみ `repoDir/master/`

**Step 4: Replace direct embedded/file loading**

`serve.go` の `loadMasterCSVs()` を:

- filesystem fixed path 直読みにせず
- `ResolveMasterSource()` の返す root を使って読む

**Step 5: Re-run tests**

```bash
cd photo-ai-go && go test ./internal/master ./cmd/photo-ai
```

Expected: PASS

**Step 6: Commit**

```bash
git add photo-ai-go/internal/master photo-ai-go/cmd/photo-ai/serve.go
git commit -m "feat: resolve masters from user-writable runtime directory"
```

### Task 4: First-Run Seed Extraction を実装する

**Files:**
- Modify: `photo-ai-go/internal/master/runtime.go`
- Modify: `photo-ai-go/internal/master/manifest.go`
- Test: `photo-ai-go/internal/master/runtime_test.go`

**Step 1: Write the failing tests**

テストケース:

- 初回起動で embedded CSV/manifest が user dir に展開される
- 既存 user master がある場合は上書きしない
- `--refresh-master-seed` 相当の明示更新のみ上書きする

**Step 2: Run tests to verify they fail**

```bash
cd photo-ai-go && go test ./internal/master -run Seed
```

**Step 3: Implement extraction**

方針:

- manifest を先に書くのではなく temp dir 展開後に atomic rename
- hash 一致で no-op
- 更新衝突時は `backup/<timestamp>/` 退避

**Step 4: Re-run tests**

```bash
cd photo-ai-go && go test ./internal/master -run Seed
```

Expected: PASS

**Step 5: Commit**

```bash
git add photo-ai-go/internal/master
git commit -m "feat: seed embedded masters into user data on first run"
```

### Task 5: CLI と Web に master source/status を表示する

**Files:**
- Modify: `photo-ai-go/cmd/photo-ai/serve.go`
- Modify: `web/index.html`
- Modify: `README.md`
- Test: `web/server_test.go`

**Step 1: Write the failing test**

`/api/master` か `/api/runtime-status` が以下を返す:

- `masterSource`
- `masterRoot`
- `masterVersion`
- `schemaVersion`
- `seedWritable`

**Step 2: Run the server test**

```bash
cd .. && go test ./web -run Master
```

Expected: FAIL

**Step 3: Implement status surface**

UI 表示:

- 「ユーザーマスタ使用中」
- 「埋め込み seed から展開」
- 「schema mismatch」

**Step 4: Re-run tests**

```bash
go test ./web -run Master
```

Expected: PASS

**Step 5: Commit**

```bash
git add photo-ai-go/cmd/photo-ai/serve.go web/index.html README.md
git commit -m "feat: expose master source and schema status"
```

### Task 6: Analyze Path で master schema compatibility を強制する

**Files:**
- Modify: `photo-engine/src/analysis.rs`
- Modify: `photo-engine/src/bin/photo-analysis-engine.rs`
- Modify: `photo-ai-go/pkg/engine/engine.go`
- Test: `photo-engine/src/analysis.rs`

**Step 1: Write the failing Rust tests**

ケース:

- supported schema と一致する master は通る
- 不一致なら `unsupported master schema` を返す
- error JSON に expected/actual を含める

**Step 2: Run tests to verify they fail**

```bash
cargo test -p photo-engine schema
```

**Step 3: Implement compatibility gate**

実装ポイント:

- `match-master` 入口で manifest を読む
- CSV 単体指定時は sidecar manifest を探す
- 見つからなければ legacy schema 0 とみなす

**Step 4: Re-run tests**

```bash
cargo test -p photo-engine schema
```

Expected: PASS

**Step 5: Commit**

```bash
git add photo-engine/src/analysis.rs photo-engine/src/bin/photo-analysis-engine.rs
git commit -m "feat: enforce master schema compatibility in analysis engine"
```

### Task 7: Legacy CSV 運用を維持したまま JSON 管理への移行口を作る

**Files:**
- Create: `photo-ai-go/internal/master/export_json.go`
- Create: `photo-ai-go/internal/master/export_json_test.go`
- Create: `common/src/master_json.rs`
- Modify: `README.md`

**Step 1: Write the failing tests**

ケース:

- CSV 群を JSON bundle に変換できる
- JSON bundle から CSV を再生成できる
- round-trip で論理行数が一致する

**Step 2: Run tests to verify they fail**

```bash
cd photo-ai-go && go test ./internal/master -run JSON
cd .. && cargo test -p photo-ai-rust master_json
```

**Step 3: Implement minimal JSON bundle**

bundle 例:

```json
{
  "schema_version": 1,
  "master_version": "2026-04-11",
  "work_types": {
    "舗装工": [ ...rows... ]
  }
}
```

この段階では runtime の正本はまだ CSV のままでよい。

**Step 4: Re-run tests**

```bash
cd photo-ai-go && go test ./internal/master -run JSON
cd .. && cargo test -p photo-ai-rust master_json
```

Expected: PASS

**Step 5: Commit**

```bash
git add photo-ai-go/internal/master common/src/master_json.rs README.md
git commit -m "feat: add json bundle import-export for masters"
```

### Task 8: Release Workflow を同一コミット固定に修正する

**Files:**
- Modify: `.github/workflows/go-release.yml`
- Modify: `.github/workflows/ci.yml`
- Modify: `photo-ai-go/Makefile`

**Step 1: Write the failing workflow expectation**

必要条件:

- `engines-latest` を取らない
- 同一 checkout から Rust engine を build
- build した engine と manifest を Go binary に embed

**Step 2: Validate current workflow is wrong**

Run:

```bash
rg -n "engines-latest|Download pre-built Rust engines" .github/workflows photo-ai-go/Makefile
```

Expected: 現在は pre-built download に依存

**Step 3: Change workflow**

最小変更:

- Windows job で Rust toolchain を導入
- `cargo build --release -p photo-engine --bin photo-analysis-engine --bin photo-tag-engine --bin photo-pdf-engine --bin photo-excel-engine`
- 生成物を `photo-ai-go/internal/engines/` にコピー

**Step 4: Smoke-check workflow config**

```bash
git diff -- .github/workflows/go-release.yml .github/workflows/ci.yml photo-ai-go/Makefile
```

Expected: download step が消え、 local build に置換

**Step 5: Commit**

```bash
git add .github/workflows/go-release.yml .github/workflows/ci.yml photo-ai-go/Makefile
git commit -m "build: embed engines built from the same commit"
```

### Task 9: End-to-End Verification

**Files:**
- Modify: `web/server_test.go`
- Create: `photo-ai-go/tests/master_runtime_e2e_test.go`
- Modify: `docs/TROUBLESHOOTING.md`

**Step 1: Write failing integration tests**

確認項目:

- 初回起動で user master seed 展開
- user が CSV を編集すると再起動後も保持
- embedded 更新後も勝手に上書きされない
- schema mismatch 時に analyze が明確に fail
- runtime status で source/version が見える

**Step 2: Run tests to verify they fail**

```bash
cd photo-ai-go && go test ./... 
cd .. && cargo test -p photo-engine
```

**Step 3: Fix remaining gaps only**

不足が出た箇所だけ最小修正する。

**Step 4: Run full verification**

```bash
cd photo-ai-go && go test ./...
cd .. && cargo test
```

Expected: PASS

**Step 5: Commit**

```bash
git add web/server_test.go photo-ai-go/tests docs/TROUBLESHOOTING.md
git commit -m "test: verify runtime master separation end to end"
```

## Notes

- まずは CSV 正本を維持し、manifest + runtime resolver を先に入れる。いきなり DB 化しない。
- JSON bundle は「内部管理への移行口」であり、第一段階の runtime 正本ではない。
- `--master` 直接指定は残すが、manifest sidecar が無い場合は legacy schema 扱いにする。
- engine 版と master 版のズレをユーザーが見える形で表示すること。
