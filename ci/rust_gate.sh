#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

MODE="full"

source "$SCRIPT_DIR/common.sh"
ensure_rustup_bin_path

CARGO_BIN="$(resolve_tool cargo)"
export CARGO_INCREMENTAL=0
export CARGO_TARGET_DIR="$ROOT_DIR/target/ci/rust"
mkdir -p "$CARGO_TARGET_DIR"

while (($#)); do
  case "$1" in
    --fast|--full|--strict)
      MODE="${1#--}"
      ;;
    -h|--help)
      cat <<'EOF'
Usage: ./ci/rust_gate.sh [--fast|--full|--strict]
EOF
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 2
      ;;
  esac
  shift
done

cd "$ROOT_DIR"
export CARGO_INCREMENTAL=0

echo "== Rust gate =="
echo "Mode: $MODE"

run() {
  echo
  echo "\$ $*"
  run_tool "$@"
}

run "$CARGO_BIN" fmt --all --check

case "$MODE" in
  fast)
    run "$CARGO_BIN" clippy --workspace --all-targets -- -D warnings
    run "$CARGO_BIN" test --workspace
    ;;
  full)
    run "$CARGO_BIN" clippy --workspace --all-targets -- -D warnings
    run "$CARGO_BIN" test --workspace
    run "$CARGO_BIN" build --workspace
    ;;
  strict)
    run "$CARGO_BIN" clippy --workspace --all-targets --all-features --locked -- -D warnings
    run "$CARGO_BIN" test --workspace --all-features --locked
    run "$CARGO_BIN" build --workspace --all-features --locked
    run "$CARGO_BIN" doc --workspace --no-deps --locked
    ;;
  *)
    echo "Unknown mode: $MODE" >&2
    exit 2
    ;;
esac

echo
echo "PASS: Rust gate"
