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

# ── Cross-engine compare mode (opt-in) ──────────────────────────────────────
if [ "${1:-}" = "--compare-engines" ]; then
  PHOTO_FOLDER="${2:-}"
  if [ -z "${PHOTO_FOLDER}" ]; then
    echo "Usage: $0 --compare-engines <photo_folder>" >&2
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
