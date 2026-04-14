# Golden-Diff Harness — Rust ↔ Go Parity (Issue #165 Stream E)

Prevents silent drift between the Rust reference implementation (`common/`)
and the Go shadow port (`photo-ai-go/internal/normalizer/`).

## Layout

```
tests/golden_diff/
├── README.md                              ← this file
└── fixtures/
    ├── temperature_normalization/
    │   ├── input.json                     ← raw OCR text inputs
    │   └── expected.json                  ← golden output (same schema for both langs)
    └── station_normalization/
        ├── input.json
        └── expected.json

common/tests/golden_diff.rs                ← Rust integration test (reads fixtures above)
photo-ai-go/internal/normalizer/golden_diff_test.go  ← Go test (reads same fixtures)
scripts/golden_diff.sh                     ← single-command runner
```

## Fixtures

| Fixture | What it tests |
|---------|---------------|
| `temperature_normalization` | `TemperatureKind::from_text` / `TemperatureKindFromText` — 13 cases covering all 5 kinds, spelling variants (初期転圧前/解放), short-text triggers (外気温), and non-matching strings |
| `station_normalization` | `Station::parse` / `StationParse` — 14 cases covering Empty, Post (with/without lane, 取付道路), Date (with/without dump number), invalid month/day → Other, and freetext → Other |

## Running

```bash
# Single command (from repo root)
./scripts/golden_diff.sh

# Rust only
cargo test -p photo-ai-common --test golden_diff

# Go only
cd photo-ai-go && go test ./internal/normalizer/ -run Golden -v

# Full Go suite
cd photo-ai-go && go test ./...

# Rust common
cargo test -p photo-ai-common
```

Note: on Windows with lld-link, add LLVM/bin to PATH first if not on CI:
```bash
export PATH="$PATH:/c/Program Files/LLVM/bin"
```

## Adding a new fixture

1. Create `tests/golden_diff/fixtures/<name>/input.json` — array of `{id, ...input_fields}`.
2. Create `tests/golden_diff/fixtures/<name>/expected.json` — array of `{id, ...output_fields}`.
   IDs must be in the same order as `input.json`.
3. Add a `Test<Name>Golden` function to `photo-ai-go/internal/normalizer/golden_diff_test.go`
   that calls the relevant Go functions and marshals results to match the expected schema.
4. Add a `#[test] fn <name>_matches_golden()` function to `common/tests/golden_diff.rs`
   that does the same for Rust.
5. Run `./scripts/golden_diff.sh` — both sides must be green before committing.

## Schema contract

`input.json` and `expected.json` are the single source of truth.
Both language runners must produce JSON that matches `expected.json` field-by-field.
The comparator is a per-row string equality check after `json.Marshal` / `serde_json::to_string`.
