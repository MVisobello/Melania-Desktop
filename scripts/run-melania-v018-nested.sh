#!/usr/bin/env bash
# Melania 0.18 nested desktop: background, taskbar, files, settings and notifications; isolated D-Bus prevents fighting with the
# host session's notification daemon. Never replaces your existing desktop.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
for bin in cargo dbus-run-session; do
  command -v "$bin" >/dev/null || { echo "Missing $bin" >&2; exit 3; }
done
if [[ -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]]; then
  echo "Run from an existing graphical session (or xvfb-run)." >&2; exit 3
fi
cargo build -p melania-compositor -p melania-panel -p melania-desktop-surface -p melania-files -p melania-settings -p melania-notifyd -p melania-notifyctl
export MELANIA_ROOT="$ROOT"
export PATH="$ROOT/target/debug:$PATH"
dbus-run-session -- bash -euo pipefail -c '
  "$MELANIA_ROOT/target/debug/melania-notifyd" &
  NOTIFY_PID=$!
  trap '\''kill "$NOTIFY_PID" 2>/dev/null || true'\'' EXIT
  "$MELANIA_ROOT/target/debug/melania-compositor" \
    --command "$MELANIA_ROOT/target/debug/melania-desktop-surface" \
    --command "$MELANIA_ROOT/target/debug/melania-panel" \
    --command "$MELANIA_ROOT/target/debug/melania-files" \
    --command "$MELANIA_ROOT/target/debug/melania-settings"
'
