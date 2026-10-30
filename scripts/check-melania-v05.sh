#!/usr/bin/env bash
# Build/typecheck/test convenience wrapper. Requires Linux, Rust/Cargo & GTK4 -dev.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
python3 scripts/static-check.py
command -v cargo >/dev/null || { echo 'Cargo assente: non è possibile verificare la compilazione.' >&2; exit 2; }
cargo fmt --all -- --check
cargo test -p melania-core -p melania-config -p melania-services -p melania-transfer -p melania-dnd
cargo check --workspace
