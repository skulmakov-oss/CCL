#!/usr/bin/env bash
set -euo pipefail

VERBOSE_EVIDENCE=0

for arg in "$@"; do
  case "$arg" in
    --verbose-evidence)
      VERBOSE_EVIDENCE=1
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

if [[ ! -d ".git" ]]; then
  echo "Run this script from the repository root." >&2
  exit 1
fi

run_step() {
  local name="$1"
  shift

  echo
  echo "==> $name"
  "$@"
}

echo "CCL demo"
echo "Repository root: $(pwd)"

run_step "CCL version" \
  cargo run -p ccl-cli -- --version

run_step "Contract check" \
  cargo run -p ccl-cli -- contract check examples/ccl-admission-task-contract.json

run_step "Repository preflight" \
  cargo run -p ccl-cli -- preflight --repo .

if [[ "$VERBOSE_EVIDENCE" == "1" ]]; then
  run_step "Validation runner" \
    cargo run -p ccl-cli -- validate run --contract examples/ccl-admission-task-contract.json --repo .

  run_step "Scope check" \
    cargo run -p ccl-cli -- scope check --contract examples/ccl-admission-task-contract.json --repo .

  run_step "Ledger verification" \
    cargo run -p ccl-cli -- ledger verify --contract examples/ccl-admission-task-contract.json --repo .
fi

run_step "Gate run" \
  cargo run -p ccl-cli -- gate run --contract examples/ccl-admission-task-contract.json --repo .

echo
echo "CCL demo completed."
echo "Expected result: gate PASS."
echo "Generated evidence artifacts are under .ccl/runs/."
