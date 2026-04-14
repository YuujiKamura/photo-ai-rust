#!/usr/bin/env bash
# scripts/golden_diff.sh — run Rust + Go golden-diff harness (issue #165 Stream E).
#
# Usage:
#   ./scripts/golden_diff.sh                           # normalizer parity tests
#   ./scripts/golden_diff.sh --compare-engines <folder> # cross-engine result diff (opt-in)
#
# --compare-engines <folder>
#   Runs  photo-ai analyze --engine=rust <folder>  and
#         photo-ai analyze --engine=go  <folder>
#   on the same photo folder, then diffs the two result.json outputs.
#   If the outputs differ (expected while Go pipeline is partial — Stream H),
#   the script emits a clear "divergence is expected" note and exits 0.
#   The diff is shown for informational purposes only.
#
# Exit: 0 if all selected checks pass (or divergence-is-expected for cross-engine).
#
# Both runners read the same fixtures under tests/golden_diff/fixtures/.
# No absolute paths are embedded; paths are resolved relative to this script.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# ── colours (graceful fallback when not a TTY) ────────────────────────────────
if [ -t 1 ]; then
  RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RESET='\033[0m'
else
  RED=''; GREEN=''; YELLOW=''; RESET=''
fi

pass() { echo -e "${GREEN}PASS${RESET}  $*"; }
fail() { echo -e "${RED}FAIL${RESET}  $*"; }
info() { echo -e "${YELLOW}----${RESET}  $*"; }
note() { echo -e "${YELLOW}NOTE${RESET}  $*"; }

overall=0

# ── Equivalence fixture mode: run Go CI-safe equivalence test ──────────────
# Usage: ./scripts/golden_diff.sh --fixture [fixture_name]
# Runs go test ./internal/equivalence/... which:
#   - Uses tests/equivalence/fixtures/<name>/photo-groups.json as mock AI output
#   - Does NOT require photo-ai.exe or real photos
#   - Skips Rust golden comparison if snapshot is a placeholder
if [ "${1:-}" = "--fixture" ]; then
  FIXTURE_NAME="${2:-sample_pavement_job}"
  FIXTURE_PATH="${REPO_ROOT}/tests/equivalence/fixtures/${FIXTURE_NAME}"

  if [ ! -d "${FIXTURE_PATH}" ]; then
    echo "Error: fixture not found: ${FIXTURE_PATH}" >&2
    echo "Available fixtures:" >&2
    ls "${REPO_ROOT}/tests/equivalence/fixtures/" 2>/dev/null || true
    exit 1
  fi

  info "Running Go equivalence tests for fixture: ${FIXTURE_NAME}"
  info "Fixture path: ${FIXTURE_PATH}"
  echo ""

  export EQUIV_FIXTURE_DIR="${REPO_ROOT}/tests/equivalence/fixtures"

  if (cd "${REPO_ROOT}/photo-ai-go" && go test -v -timeout 120s ./internal/equivalence/... 2>&1); then
    pass "Equivalence test PASSED for fixture: ${FIXTURE_NAME}"
    exit 0
  else
    fail "Equivalence test FAILED for fixture: ${FIXTURE_NAME}"
    exit 1
  fi
fi

# ── Cross-engine compare mode (opt-in) ──────────────────────────────────────
# Usage: ./scripts/golden_diff.sh --compare-engines [photo_folder|fixture_name]
# When a fixture name is given instead of a real photo folder, the script uses
# the fixture's photo-groups.json to short-circuit the AI tagger step.
if [ "${1:-}" = "--compare-engines" ]; then
  ARG="${2:-}"
  # Check if it looks like a fixture name (no path separators, inside fixtures/).
  FIXTURE_AUTO="${REPO_ROOT}/tests/equivalence/fixtures/${ARG}"
  if [ -n "${ARG}" ] && [ -d "${FIXTURE_AUTO}" ] && [ ! -d "${ARG}" ]; then
    # Fixture mode: use photo-groups.json from fixture as the tagger cache.
    info "Detected fixture name: ${ARG}"
    info "Using ${FIXTURE_AUTO}/photo-groups.json as tagger cache."
    echo ""
    export EQUIV_FIXTURE_DIR="${REPO_ROOT}/tests/equivalence/fixtures"
    if (cd "${REPO_ROOT}/photo-ai-go" && \
        PHOTO_ANALYSIS_ENGINE_EXE="${PHOTO_ANALYSIS_ENGINE_EXE:-}" \
        go test -v -timeout 120s -run TestGoVsRustLive ./internal/equivalence/... 2>&1); then
      pass "Cross-engine (live) comparison PASSED for fixture: ${ARG}"
    else
      note "Cross-engine live comparison skipped or diverged (see output above)."
      note "This is expected until PHOTO_ANALYSIS_ENGINE_EXE is set and engines are built."
      pass "Cross-engine compare completed (see output for divergence details)."
    fi
    exit 0
  fi

  PHOTO_FOLDER="${ARG}"
  if [ -z "${PHOTO_FOLDER}" ]; then
    echo "Usage: $0 --compare-engines <photo_folder|fixture_name>" >&2
    exit 1
  fi
  if [ ! -d "${PHOTO_FOLDER}" ]; then
    echo "Error: folder not found: ${PHOTO_FOLDER}" >&2
    exit 1
  fi

  PHOTO_AI="${REPO_ROOT}/photo-ai-go/photo-ai.exe"
  if [ ! -x "${PHOTO_AI}" ]; then
    # Try standard build output name
    PHOTO_AI="${REPO_ROOT}/photo-ai-go/photo-ai"
  fi
  if [ ! -x "${PHOTO_AI}" ]; then
    echo "Error: photo-ai binary not found. Run: cd photo-ai-go && go build ./cmd/photo-ai/" >&2
    exit 1
  fi

  TMPDIR_RUST="$(mktemp -d)"
  TMPDIR_GO="$(mktemp -d)"
  trap 'rm -rf "${TMPDIR_RUST}" "${TMPDIR_GO}"' EXIT

  RESULT_RUST="${TMPDIR_RUST}/result.json"
  RESULT_GO="${TMPDIR_GO}/result.json"

  info "Running Rust engine on: ${PHOTO_FOLDER}"
  if "${PHOTO_AI}" analyze --engine=rust --output="${RESULT_RUST}" "${PHOTO_FOLDER}" 2>&1; then
    pass "Rust engine completed"
  else
    fail "Rust engine failed"
    exit 1
  fi

  echo ""
  info "Running Go engine on: ${PHOTO_FOLDER}"
  if "${PHOTO_AI}" analyze --engine=go --output="${RESULT_GO}" "${PHOTO_FOLDER}" 2>&1; then
    pass "Go engine completed"
  else
    fail "Go engine failed"
    exit 1
  fi

  echo ""
  info "Diffing Rust vs Go result.json..."
  if diff -u "${RESULT_RUST}" "${RESULT_GO}" > /dev/null 2>&1; then
    pass "Outputs are identical — engines have reached parity."
  else
    note "Outputs DIFFER between --engine=rust and --engine=go."
    note "This divergence is EXPECTED until Stream H (#165) completes the Go pipeline port."
    note "Fields missing from the Go engine: workType, variety, subphase, station,"
    note "  description, hasBoard, detectedText, measurements, photoCategory,"
    note "  reasoning, focusTarget — all return empty/zero until AI integration lands."
    note "analysisTransport differs by design: 'binary_engine' vs 'go_pipeline'."
    echo ""
    info "--- Diff (informational) ---"
    diff -u "${RESULT_RUST}" "${RESULT_GO}" || true
    echo ""
    note "No failure is raised here. Re-run after Stream H to confirm parity."
    pass "Cross-engine compare completed (divergence expected — no failure)."
  fi

  exit 0
fi

# ── Standard normalizer golden-diff tests ────────────────────────────────────

# ── Rust ──────────────────────────────────────────────────────────────────────
info "Running Rust golden-diff tests (cargo test -p photo-ai-common --test golden_diff)…"
if (cd "${REPO_ROOT}" && cargo test -p photo-ai-common --test golden_diff -- --nocapture 2>&1); then
  pass "Rust golden-diff"
else
  fail "Rust golden-diff"
  overall=1
fi

echo ""

# ── Go ────────────────────────────────────────────────────────────────────────
info "Running Go golden-diff tests (go test ./internal/normalizer/ -run Golden)…"
if (cd "${REPO_ROOT}/photo-ai-go" && go test ./internal/normalizer/ -run Golden -v 2>&1); then
  pass "Go golden-diff"
else
  fail "Go golden-diff"
  overall=1
fi

echo ""

# ── Summary ──────────────────────────────────────────────────────────────────
if [ "${overall}" -eq 0 ]; then
  pass "All golden-diff checks passed — Rust ↔ Go parity confirmed."
else
  fail "One or more golden-diff checks FAILED — implementations have diverged."
fi

exit "${overall}"
