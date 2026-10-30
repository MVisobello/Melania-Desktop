#!/usr/bin/env bash
# The two outcomes are deliberately separate. Static OK != compiled desktop.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
python3 -m unittest discover -s tests -p 'test_*.py' -v
python3 -m unittest discover -s preview-lab -p 'test_preview.py' -v
bash -n scripts/run-melania-v018-nested.sh
bash -n scripts/check-melania-v019.sh
printf '\nSTATIC CHECKS: PASSED\n'
if ! command -v cargo >/dev/null 2>&1; then
    echo 'RUST COMPILATION: NOT RUN (Cargo unavailable)' >&2
    exit 3
fi
cargo test -p melania-desktop-core
cargo check -p melania-desktop-surface -p melania-panel
printf '\nRUST UNIT TESTS AND GTK CHECK: PASSED\n'
