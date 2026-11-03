#!/usr/bin/env bash
# Nested preview only: never replaces the host desktop or installs a login session.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

preflight() {
  local missing=0 bin
  for bin in cargo dbus-run-session bash; do
    if ! command -v "$bin" >/dev/null 2>&1; then
      echo "MISSING_DEPENDENCY: $bin" >&2
      missing=1
    fi
  done
  if [[ -z "${DISPLAY:-}" && -z "${WAYLAND_DISPLAY:-}" ]]; then
    echo "NO_HOST_DISPLAY: nested preview requires an existing graphical session." >&2
    missing=1
  fi
  if [[ ! -f "$ROOT/Cargo.toml" ]]; then
    echo "MISSING_WORKSPACE: $ROOT/Cargo.toml" >&2
    missing=1
  fi
  if (( missing )); then return 3; fi
  echo "NESTED_PREFLIGHT_OK: host display and prerequisites detected (not a native session)"
}

case "${1:-}" in
  --preflight) [[ $# -eq 1 ]] || exit 2; preflight; exit $? ;;
  "") ;;
  *) echo "Usage: $0 [--preflight]" >&2; exit 2 ;;
esac
preflight
cargo build -p melania-compositor -p melania-panel -p melania-desktop-surface -p melania-files -p melania-settings -p melania-notifyd -p melania-notifyctl
export MELANIA_ROOT="$ROOT"
export PATH="$ROOT/target/debug:$PATH"
# Keep the notification daemon inside an isolated D-Bus session. Kill and reap
# it when the compositor exits, including on an interrupted preview.
dbus-run-session -- bash -euo pipefail -c '
  "$MELANIA_ROOT/target/debug/melania-notifyd" &
  NOTIFY_PID=$!
  cleanup() {
    trap - EXIT
    kill "$NOTIFY_PID" 2>/dev/null || true
    wait "$NOTIFY_PID" 2>/dev/null || true
  }
  trap cleanup EXIT
  # Desktop and panel are the only essential startup clients. Opening Files
  # and Settings on every boot delays first interaction and is not normal DE
  # behavior. Keep the previous four-window layout opt-in for visual tests.
  launch_args=(
    --command "$MELANIA_ROOT/target/debug/melania-desktop-surface"
    --command "$MELANIA_ROOT/target/debug/melania-panel"
  )
  if [[ "${MELANIA_PREVIEW_OPEN_APPS:-0}" == "1" ]]; then
    launch_args+=(
      --command "$MELANIA_ROOT/target/debug/melania-files"
      --command "$MELANIA_ROOT/target/debug/melania-settings"
    )
  fi
  "$MELANIA_ROOT/target/debug/melania-compositor" "${launch_args[@]}"
'
