#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
python3 scripts/static-check.py
if ! command -v cargo >/dev/null; then
  echo 'ATTENZIONE: cargo assente; i test Rust NON sono stati eseguiti.' >&2
  exit 2
fi
cargo test -p melania-shell-model
cargo check --workspace
