# Fixture: cmd_analyze_smoke

Cached result.json snapshots for cross-engine comparison testing.

## Files

- `cached_result_rust.json` — reference output from `--engine=rust` (Rust DLL path).
  Contains full semantic fields: workType, variety, subphase, station, etc.
- `cached_result_go.json` — reference output from `--engine=go` (Go-native pipeline).
  Currently a skeleton: AI-tagged fields are empty because the Go pipeline's AI
  integration is incomplete pending Stream H (#165).

## Schema contract

Both files share the same key set (engine.AnalysisResult JSON tags).
Key difference: `analysisTransport` is `"binary_engine"` for Rust and `"go_pipeline"` for Go.

## Known divergences (expected until Stream H lands)

| Field            | Rust                  | Go (current)  |
|------------------|-----------------------|---------------|
| workType         | populated by DLL      | empty string  |
| variety          | populated by DLL      | empty string  |
| subphase         | populated by DLL      | empty string  |
| station          | populated by DLL      | empty string  |
| description      | populated by DLL      | empty string  |
| hasBoard         | populated by DLL      | false         |
| detectedText     | populated by DLL      | empty string  |
| measurements     | populated by DLL      | empty string  |
| photoCategory    | populated by DLL      | empty string  |
| reasoning        | populated by DLL      | empty string  |
| focusTarget      | populated by DLL      | empty string  |

These divergences are expected; see `--compare-engines` note in golden_diff.sh.
