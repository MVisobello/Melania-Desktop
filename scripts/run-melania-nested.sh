#!/usr/bin/env bash
# Experimental compositor in a host window. Does not replace your login session.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if [[ "$(uname -s)" != Linux ]]; then echo 'Melania richiede Linux.' >&2; exit 2; fi
if [[ -z "${XDG_RUNTIME_DIR:-}" || ! -d "$XDG_RUNTIME_DIR" ]]; then
  echo 'Manca XDG_RUNTIME_DIR: avviare dentro una normale sessione Linux.' >&2; exit 2
fi
if [[ -z "${WAYLAND_DISPLAY:-}" && -z "${DISPLAY:-}" ]]; then
  echo 'La modalità annidata richiede una sessione grafica host (Wayland o X11).' >&2; exit 2
fi
command -v cargo >/dev/null || { echo 'Installa Rust/Cargo: https://rustup.rs' >&2; exit 2; }
echo '[Melania] Compilo il compositore e Melania Files…'
cargo build -p melania-compositor -p melania-files
printf '\n[Melania] Avvio annidato: la finestra host rimane intatta.\n'
printf '[Melania] Il terminale mostrerà WAYLAND_DISPLAY=melania-N per aprire altri client.\n'
exec "$ROOT/target/debug/melania-compositor" --command "$ROOT/target/debug/melania-files"
