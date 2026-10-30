#!/usr/bin/env bash
# Melania 0.15 — executable gate. No installs, no sudo, no changes to system settings.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
echo '[1/5] Static structural tests (NOT Rust execution)'
python3 -m unittest discover -s tests -p 'test_*.py' -v
echo '[2/5] X11 prototype filesystem tests (NOT new GTK UI)'
python3 -m unittest discover -s preview-lab -p 'test_preview.py' -v
echo '[3/5] Native prerequisites'
if ! command -v cargo >/dev/null; then
  echo 'BLOCKED: Cargo is not installed. Steps 4 and 5 were NOT executed.' >&2
  exit 3
fi
if ! pkg-config --exists gtk4; then
  echo 'BLOCKED: GTK 4 development files are missing. Steps 4 and 5 were NOT executed.' >&2
  exit 3
fi
echo '[4/5] Rust-only modules'
cargo check -p melania-transfer -p melania-dnd -p melania-shell-model -p melania-files-core --all-targets
cargo test -p melania-transfer -p melania-dnd -p melania-shell-model -p melania-files-core
echo '[5/5] GTK4 and compositor compilation'
cargo check -p melania-files -p melania-settings -p melania-panel -p melania-compositor --all-targets
echo 'PASS: compilation gates succeeded. Runtime Wayland/clipboard/DnD integration STILL requires a separate GUI smoke test.'
