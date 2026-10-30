#!/usr/bin/env bash
# Real build-and-test gate. Never claim a successful build without Cargo.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
python3 scripts/audit-build.py --json reports/offline-audit-v014.json
python3 -m unittest discover -s tests -p 'test_release_invariants.py' -v
if ! command -v cargo >/dev/null 2>&1; then
  echo 'ERRORE: Cargo non installato. Build Rust non eseguita.' >&2
  exit 30
fi
cargo test -p melania-files-core -p melania-transfer -p melania-shell-model -p melania-dnd
cargo check -p melania-files -p melania-panel -p melania-settings -p melania-preview
cargo check -p melania-compositor
printf 'Compilazione e test Rust completati con successo.\n'
