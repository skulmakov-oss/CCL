#!/usr/bin/env bash

ensure_rustup_bin_path() {
  local rustup_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
  if [ -d "$rustup_bin" ]; then
    PATH="$rustup_bin:$PATH"
    export PATH
  fi
}

ps_quote() {
  local value="$1"
  value="${value//\'/\'\'}"
  printf "'%s'" "$value"
}

resolve_tool() {
  local tool="$1"
  local resolved=""

  if resolved="$(command -v "$tool" 2>/dev/null)"; then
    printf '%s\n' "$resolved"
    return 0
  fi

  if command -v powershell.exe >/dev/null 2>&1; then
    resolved="$(
      powershell.exe -NoProfile -Command "(Get-Command '$tool' -ErrorAction SilentlyContinue).Source" 2>/dev/null || true
    )"
    resolved="${resolved//$'\r'/}"
    resolved="${resolved//$'\n'/}"

    if [ -n "$resolved" ]; then
      printf '%s\n' "$resolved"
      return 0
    fi
  fi

  return 1
}

run_tool() {
  local tool_path="$1"
  shift

  if [ -z "$tool_path" ]; then
    return 1
  fi

  case "$tool_path" in
    /*)
      case "$tool_path" in
        /usr/*|/bin/*|/opt/*|/nix/*)
          "$tool_path" "$@"
          return $?
          ;;
        *)
          if command -v powershell.exe >/dev/null 2>&1; then
            local ps_command=""
            if [ -n "${CARGO_INCREMENTAL:-}" ]; then
              ps_command+="\$env:CARGO_INCREMENTAL = $(ps_quote "$CARGO_INCREMENTAL"); "
            fi
            if [ -n "${CARGO_TARGET_DIR:-}" ]; then
              ps_command+="\$env:CARGO_TARGET_DIR = $(ps_quote "$CARGO_TARGET_DIR"); "
            fi
            ps_command+="& $(ps_quote "$tool_path")"
            local arg
            for arg in "$@"; do
              ps_command+=" $(ps_quote "$arg")"
            done
            powershell.exe -NoProfile -Command "$ps_command"
            return $?
          fi
          ;;
      esac
      ;;
    [A-Za-z]:\\*|[A-Za-z]:/*|*\\*)
      if command -v powershell.exe >/dev/null 2>&1; then
        local ps_command=""
        if [ -n "${CARGO_INCREMENTAL:-}" ]; then
          ps_command+="\$env:CARGO_INCREMENTAL = $(ps_quote "$CARGO_INCREMENTAL"); "
        fi
        if [ -n "${CARGO_TARGET_DIR:-}" ]; then
          ps_command+="\$env:CARGO_TARGET_DIR = $(ps_quote "$CARGO_TARGET_DIR"); "
        fi
        ps_command+="& $(ps_quote "$tool_path")"
        local arg
        for arg in "$@"; do
          ps_command+=" $(ps_quote "$arg")"
        done
        powershell.exe -NoProfile -Command "$ps_command"
        return $?
      fi
      ;;
  esac

  "$tool_path" "$@"
}
