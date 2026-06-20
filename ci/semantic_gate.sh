#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

MODE="full"
SMC_BIN="${SMC_BIN:-smc}"
SMC_COMPILE_OUTPUT_FLAG="${SMC_COMPILE_OUTPUT_FLAG:--o}"
SEMANTIC_REQUIRE_SMC="${SEMANTIC_REQUIRE_SMC:-0}"

source "$SCRIPT_DIR/common.sh"
ensure_rustup_bin_path

while (($#)); do
  case "$1" in
    --fast|--full|--strict)
      MODE="${1#--}"
      ;;
    -h|--help)
      cat <<'EOF'
Usage: ./ci/semantic_gate.sh [--fast|--full|--strict]
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

echo "== Semantic gate =="
echo "Mode: $MODE"

if [ ! -d examples ]; then
  echo "SKIP: examples/ directory not found"
  exit 0
fi

mapfile -t sm_files < <(find examples -maxdepth 1 -type f -name '*.sm' | sort)

if [ "${#sm_files[@]}" -eq 0 ]; then
  echo "SKIP: no .sm files found in examples/"
  exit 0
fi

mkdir -p target/semantic-ci

if [[ "$SMC_BIN" != */* && "$SMC_BIN" != *\\* ]]; then
  SMC_BIN="$(resolve_tool "$SMC_BIN" 2>/dev/null || true)"
fi

if [ -z "$SMC_BIN" ]; then
  if [ "$SEMANTIC_REQUIRE_SMC" = "1" ] || [ "$MODE" = "strict" ]; then
    echo "FAIL: semantic gate requires smc, but it is not available" >&2
    exit 2
  fi

  echo "WARN: smc not available; semantic gate skipped"
  echo "SKIP: no semantic verification was run"
  exit 0
fi

echo "smc: $SMC_BIN"

for source_file in "${sm_files[@]}"; do
  base_name="$(basename "$source_file" .sm)"
  output_file="target/semantic-ci/${base_name}.sem"

  echo
  echo "Checking: $source_file"

  case "$MODE" in
    fast|full|strict)
      run_tool "$SMC_BIN" check "$source_file"
      run_tool "$SMC_BIN" compile "$source_file" "$SMC_COMPILE_OUTPUT_FLAG" "$output_file"
      run_tool "$SMC_BIN" verify "$output_file"
      if [ "$MODE" = "full" ] || [ "$MODE" = "strict" ]; then
        run_tool "$SMC_BIN" run "$output_file"
      fi
      ;;
    *)
      echo "Unknown mode: $MODE" >&2
      exit 2
      ;;
  esac

  echo "PASS: $source_file"
done

echo
echo "PASS: Semantic gate"
