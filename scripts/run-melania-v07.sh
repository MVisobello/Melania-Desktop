#!/usr/bin/env bash
# Run entirely nested in a host Linux desktop; never replace a login session.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
[[ "$(uname -s)" == Linux ]] || { echo 'Melania richiede Linux' >&2; exit 2; }
[[ -n "${XDG_RUNTIME_DIR:-}" && -d "$XDG_RUNTIME_DIR" ]] || { echo 'XDG_RUNTIME_DIR assente: avvia nella sessione grafica' >&2; exit 2; }
[[ -n "${WAYLAND_DISPLAY:-}" || -n "${DISPLAY:-}" ]] || { echo 'Sessione grafica host non rilevata' >&2; exit 2; }
command -v cargo >/dev/null || { echo 'Rust/Cargo assenti: https://rustup.rs' >&2; exit 2; }
pkg-config --exists gtk4 || { echo 'GTK4 sviluppo mancante: sudo apt install libgtk-4-dev' >&2; exit 2; }
pkg-config --exists gtk4-layer-shell-0 || { echo 'gtk4-layer-shell sviluppo mancante: installa libgtk4-layer-shell-dev' >&2; exit 2; }
printf '%s\n' '[Melania 0.7] Compilazione (potrebbero emergere incompatibilità delle API da correggere)...'
cargo build --workspace
printf '%s\n' '[Melania 0.7] Avvio annidato; chiudi la finestra host per terminare.'
exec "$ROOT/target/debug/melania-compositor" \
  --command "$ROOT/target/debug/melania-panel" \
  --command "$ROOT/target/debug/melania-files" \
  --command "$ROOT/target/debug/melania-settings"
