#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
echo '== Melania 0.4 / controlli =='
if ! command -v cargo >/dev/null; then
 echo 'Rust/Cargo non installato; visita https://rustup.rs' >&2
 exit 1
fi
if ! command -v pkg-config >/dev/null || ! pkg-config --exists gtk4; then
 echo 'GTK 4 di sviluppo non disponibile (libgtk-4-dev su Debian/Ubuntu).' >&2
 exit 1
fi
cargo test -p melania-core -p melania-config -p melania-services -p melania-transfer -p melania-dnd
cargo check --workspace
printf 'Verifiche completate. Prototipo: cargo run -p melania-preview\n'
