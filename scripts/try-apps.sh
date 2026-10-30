#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
cargo build --workspace
export PATH="$ROOT/target/debug:$PATH"
printf 'Avvio Melania Shell come normale finestra; per uscire chiudila.\n'
exec "$ROOT/target/debug/melania-preview"
