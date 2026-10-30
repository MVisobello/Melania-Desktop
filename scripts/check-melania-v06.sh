#!/usr/bin/env bash
# Must be run on a Linux desktop with Rust and GTK4 development libraries.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
command -v cargo >/dev/null || { echo 'Rust/Cargo non installati: https://rustup.rs' >&2; exit 2; }
command -v pkg-config >/dev/null || { echo 'Manca pkg-config.' >&2; exit 2; }
pkg-config --exists gtk4 || { echo 'Installa le librerie GTK4 di sviluppo (libgtk-4-dev).' >&2; exit 2; }
echo '[Melania] Unit test senza UI (DnD, trasferimenti, geometria finestre)'
cargo test -p melania-core -p melania-transfer -p melania-dnd -p melania-config -p melania-services
echo '[Melania] Controllo di compilazione della sessione Wayland e delle applicazioni GTK'
cargo check --workspace
echo '[Melania] Controlli superati. Per il test grafico: ./scripts/run-melania-v06.sh'
