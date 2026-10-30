#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
python3 scripts/check-v026-offline.py
if ! command -v cargo >/dev/null 2>&1; then
  echo 'ERRORE: Cargo non installato. Nessuna compilazione/test Rust eseguiti.' >&2
  exit 2
fi
cargo test -p melania-workspaces -p melania-control
if ! pkg-config --exists gtk4; then
  echo 'ERRORE: gtk4-dev non disponibile. Compilazione shell incompleta.' >&2
  exit 3
fi
cargo check -p melania-panel --all-targets
cargo check -p melania-compositor --all-targets
printf 'Core, panel e compositor compilati. Eseguire anche smoke-nested-wayland.sh per la verifica grafica.\n'
