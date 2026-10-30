#!/usr/bin/env bash
# Starts three actual GTK clients inside a nested test compositor.
# Does NOT replace the user's current desktop or display manager.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if [[ "$(uname -s)" != Linux ]]; then
  echo 'Melania richiede Linux.' >&2; exit 2
fi
if [[ -z "${XDG_RUNTIME_DIR:-}" || ! -d "$XDG_RUNTIME_DIR" ]]; then
  echo 'Avvia questo script dentro una sessione Linux (XDG_RUNTIME_DIR assente).' >&2; exit 2
fi
if [[ -z "${WAYLAND_DISPLAY:-}" && -z "${DISPLAY:-}" ]]; then
  echo 'È necessaria una sessione grafica Wayland o X11 come host.' >&2; exit 2
fi
command -v cargo >/dev/null || { echo 'Installa Rust: https://rustup.rs' >&2; exit 2; }
echo '[Melania 0.6] Compilazione workspace…'
cargo build --workspace
printf '%s\n' '[Melania 0.6] Avvio annidato: chiudi la finestra host per terminare.'
exec "$ROOT/target/debug/melania-compositor" \
  --command "$ROOT/target/debug/melania-preview" \
  --command "$ROOT/target/debug/melania-files" \
  --command "$ROOT/target/debug/melania-settings"
