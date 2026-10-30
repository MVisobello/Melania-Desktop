#!/usr/bin/env bash
# La prova Rust/GTK fallisce chiaramente quando gli strumenti non sono presenti.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
command -v cargo >/dev/null || { echo 'BLOCCATO: cargo non installato' >&2; exit 3; }
command -v pkg-config >/dev/null || { echo 'BLOCCATO: pkg-config assente' >&2; exit 3; }
pkg-config --exists gtk4 || { echo 'BLOCCATO: GTK4 devel mancante' >&2; exit 3; }
pkg-config --exists gtk4-layer-shell-0 || { echo 'BLOCCATO: gtk4-layer-shell devel mancante' >&2; exit 3; }
printf '\n=== Logica shell + protocollo IPC ===\n'
cargo test -p melania-shell-model -p melania-control
printf '\n=== Verifica GTK + compositor ===\n'
cargo check -p melania-panel -p melania-compositor
printf '\n=== SUCCESSO: compilazione superata ===\n'
