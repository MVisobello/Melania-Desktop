#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
python3 -m unittest discover -s tests -p 'test_*.py' -v
for script in scripts/run-melania-v018-nested.sh scripts/run-melania-v016-nested.sh; do
    bash -n "$script"
done
python3 - <<'PYCODE'
from pathlib import Path
from struct import unpack
with open('melania-desktop-surface/assets/wallpaper.png', 'rb') as f:
    header = f.read(24)
assert header[:8] == b"\x89PNG\r\n\x1a\n"
assert unpack('>II', header[16:24]) == (1600, 900)
assert Path('melania-desktop-surface/src/main.rs').is_file()
print('Desktop asset and crate files: OK')
PYCODE
if command -v cargo >/dev/null 2>&1; then
    echo 'Cargo detected: running the actual GTK module check'
    cargo check -p melania-desktop-surface
else
    echo 'Cargo unavailable. Structural checks passed; compilation not performed.'
fi
