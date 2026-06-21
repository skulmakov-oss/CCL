#!/usr/bin/env bash
# Keep this file LF-normalized for Bash compatibility across shells.
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

DEMO_CONTRACT="${CCL_DEMO_CONTRACT:-examples/ccl-admission-task-contract.json}"

resolve_cargo() {
  if command -v cargo >/dev/null 2>&1; then
    command -v cargo
    return 0
  fi

  if command -v cargo.exe >/dev/null 2>&1; then
    command -v cargo.exe
    return 0
  fi

  if [[ -x "${HOME}/.cargo/bin/cargo.exe" ]]; then
    printf '%s\n' "${HOME}/.cargo/bin/cargo.exe"
    return 0
  fi

  if command -v where.exe >/dev/null 2>&1; then
    local cargo_path
    cargo_path="$(where.exe cargo 2>/dev/null | head -n1 | tr -d '\r')"
    if [[ -n "${cargo_path}" ]]; then
      if command -v cygpath >/dev/null 2>&1; then
        cygpath -u "${cargo_path}"
      else
        printf '%s\n' "${cargo_path}"
      fi
      return 0
    fi
  fi

  return 1
}

CARGO_BIN="$(resolve_cargo)" || {
  echo "Unable to locate cargo." >&2
  exit 1
}

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
  "$CARGO_BIN" run -p ccl-cli -- --version

run_step "Contract check" \
  "$CARGO_BIN" run -p ccl-cli -- contract check "$DEMO_CONTRACT"

run_step "Repository preflight" \
  "$CARGO_BIN" run -p ccl-cli -- preflight --repo .

if [[ "$VERBOSE_EVIDENCE" == "1" ]]; then
  run_step "Validation runner" \
    "$CARGO_BIN" run -p ccl-cli -- validate run --contract "$DEMO_CONTRACT" --repo .

  run_step "Scope check" \
    "$CARGO_BIN" run -p ccl-cli -- scope check --contract "$DEMO_CONTRACT" --repo .

  run_step "Ledger verification" \
    "$CARGO_BIN" run -p ccl-cli -- ledger verify --contract "$DEMO_CONTRACT" --repo .
fi

run_step "Gate run" \
  "$CARGO_BIN" run -p ccl-cli -- gate run --contract "$DEMO_CONTRACT" --repo .

echo
echo "CCL demo completed."
echo "Expected result: gate PASS."
echo "Generated evidence artifacts are under .ccl/runs/."
