#!/usr/bin/env bash
# Start a nested session in the current graphical desktop, not a display manager.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
[[ -d "${XDG_RUNTIME_DIR:-/nonexistent}" ]] || { echo 'XDG_RUNTIME_DIR non disponibile' >&2; exit 2; }
[[ -n "${WAYLAND_DISPLAY:-}${DISPLAY:-}" ]] || { echo 'Apri questo script da una sessione grafica Linux' >&2; exit 2; }
./scripts/check-melania-v08.sh
cargo build --workspace
exec "$ROOT/target/debug/melania-compositor" \
  --command "$ROOT/target/debug/melania-panel" \
  --command "$ROOT/target/debug/melania-files" \
  --command "$ROOT/target/debug/melania-settings"
