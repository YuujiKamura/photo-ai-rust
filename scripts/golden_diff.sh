#!/usr/bin/env bash
# scripts/golden_diff.sh — run Rust + Go golden-diff harness (issue #165 Stream E).
#
# Usage:  ./scripts/golden_diff.sh
# Exit:   0 if both sides pass; non-zero otherwise.
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

overall=0

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
