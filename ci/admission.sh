#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

MODE="full"
SEMANTIC_REQUIRE_SMC="${SEMANTIC_REQUIRE_SMC:-0}"

usage() {
  cat <<'EOF'
Usage: ./ci/admission.sh [--fast|--full|--strict|--rust-only|--semantic-only]
EOF
}

while (($#)); do
  case "$1" in
    --fast|--full|--strict|--rust-only|--semantic-only)
      MODE="${1#--}"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

run_step() {
  local label="$1"
  shift

  echo
  echo "[$label]"
  if "$@"; then
    echo "PASS: $label"
  else
    echo "FAIL: $label" >&2
    return 1
  fi
}

echo "== Semantic Admission Gate =="
echo "Mode: $MODE"

if [ "$MODE" = "strict" ]; then
  SEMANTIC_REQUIRE_SMC=1
fi
if [ "$MODE" = "rust-only" ]; then
  SEMANTIC_CHECK_ENABLED=0
else
  SEMANTIC_CHECK_ENABLED=1
fi
export SEMANTIC_CHECK_ENABLED
export SEMANTIC_REQUIRE_SMC

case "$MODE" in
  rust-only)
    run_step "Environment gate" "$SCRIPT_DIR/env_check.sh"
    run_step "Rust gate" "$SCRIPT_DIR/rust_gate.sh"
    ;;
  semantic-only)
    run_step "Environment gate" "$SCRIPT_DIR/env_check.sh"
    run_step "Semantic gate" "$SCRIPT_DIR/semantic_gate.sh"
    ;;
  fast)
    run_step "Environment gate" "$SCRIPT_DIR/env_check.sh"
    run_step "Rust gate" "$SCRIPT_DIR/rust_gate.sh" --fast
    run_step "Semantic gate" "$SCRIPT_DIR/semantic_gate.sh" --fast
    ;;
  full)
    run_step "Environment gate" "$SCRIPT_DIR/env_check.sh"
    run_step "Rust gate" "$SCRIPT_DIR/rust_gate.sh"
    run_step "Semantic gate" "$SCRIPT_DIR/semantic_gate.sh"
    ;;
  strict)
    run_step "Environment gate" "$SCRIPT_DIR/env_check.sh"
    run_step "Rust gate" "$SCRIPT_DIR/rust_gate.sh" --strict
    run_step "Semantic gate" "$SCRIPT_DIR/semantic_gate.sh" --strict
    ;;
  *)
    echo "Unknown mode: $MODE" >&2
    exit 2
    ;;
esac

echo
echo "== Admission passed =="
