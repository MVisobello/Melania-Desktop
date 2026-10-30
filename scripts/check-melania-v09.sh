#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
[[ "$(uname -s)" == Linux ]] || { echo 'Melania 0.9 richiede Linux' >&2; exit 2; }
command -v cargo >/dev/null || { echo 'Installa Rust/Cargo: https://rustup.rs' >&2; exit 2; }
command -v pkg-config >/dev/null || { echo 'pkg-config non disponibile' >&2; exit 2; }
pkg-config --exists gtk4 || { echo 'GTK 4 SDK mancante (libgtk-4-dev)' >&2; exit 2; }
pkg-config --exists gtk4-layer-shell-0 || { echo 'GTK4 Layer Shell SDK mancante (libgtk4-layer-shell-dev)' >&2; exit 2; }
python3 scripts/static-check.py
cargo test -p melania-filectl -p melania-control -p melania-shell-model -p melania-core -p melania-dnd -p melania-transfer
cargo check --workspace
printf '%s\n' 'Melania 0.9: cargo test e cargo check completati.'
