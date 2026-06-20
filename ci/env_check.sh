#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

source "$SCRIPT_DIR/common.sh"
ensure_rustup_bin_path

fail() {
  echo "FAIL: environment error: $*" >&2
  exit 2
}

require_tool() {
  local tool_name="$1"
  local tool_path

  tool_path="$(resolve_tool "$tool_name" 2>/dev/null || true)"
  [ -n "$tool_path" ] || fail "$tool_name not found in PATH"
  printf '%s\n' "$tool_path"
}

SEMANTIC_REQUIRE_SMC="${SEMANTIC_REQUIRE_SMC:-0}"
SEMANTIC_CHECK_ENABLED="${SEMANTIC_CHECK_ENABLED:-1}"

echo "== Environment gate =="

git_path="$(require_tool git)"
rustc_path="$(require_tool rustc)"
cargo_path="$(require_tool cargo)"

echo "INFO: git => $git_path"
echo "INFO: rustc => $rustc_path"
echo "INFO: cargo => $cargo_path"

if [ "$SEMANTIC_CHECK_ENABLED" = "1" ]; then
  if smc_path="$(resolve_tool smc 2>/dev/null || true)"; then
    if [ -n "$smc_path" ]; then
      echo "INFO: smc => $smc_path"
    fi
  fi

  if [ -z "${smc_path:-}" ]; then
    if [ "$SEMANTIC_REQUIRE_SMC" = "1" ]; then
      fail "smc not found in PATH and strict admission requires it"
    fi
    echo "WARN: smc not found in PATH; semantic gate will skip"
  fi
fi

[ -f "$ROOT_DIR/Cargo.toml" ] || fail "Cargo.toml is missing"
[ -d "$ROOT_DIR/ci" ] || fail "ci/ directory is missing"

if [ -d "$ROOT_DIR/examples" ]; then
  echo "INFO: examples/ directory found"
else
  echo "SKIP: examples/ directory is missing"
fi

echo "PASS: environment looks ready"
