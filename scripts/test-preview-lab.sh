#!/usr/bin/env bash
# Reproducible GUI smoke test on a private Xvfb display. No desktop installs.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORT="${1:-$ROOT/preview-lab/melania-preview-tested.png}"
command -v Xvfb >/dev/null || { echo 'Xvfb required'; exit 2; }
python3 -c 'import tkinter, PIL' 2>/dev/null || { echo 'Python needs tkinter, Pillow and pyautogui'; exit 2; }
D="$(mktemp -d -t melania-gui-test-XXXXXX)"
export DISPLAY=:98
export XAUTHORITY="$D/xauth"
touch "$XAUTHORITY"
cleanup() { [[ -n "${GUI_PID:-}" ]] && kill "$GUI_PID" 2>/dev/null || :; [[ -n "${XVFB_PID:-}" ]] && kill "$XVFB_PID" 2>/dev/null || :; rm -rf "$D"; }
trap cleanup EXIT
Xvfb :98 -screen 0 1280x800x24 -ac -nolisten tcp >"$D/xvfb.log" 2>&1 & XVFB_PID=$!
sleep 1
python3 "$ROOT/preview-lab/melania_preview.py" --demo-dir "$D/demo" >"$D/gui.log" 2>&1 & GUI_PID=$!
sleep 2
kill -0 "$GUI_PID" || { cat "$D/gui.log"; exit 1; }
mkdir -p "$(dirname "$REPORT")"
SCREENSHOT="$REPORT" DEMO="$D/demo" python3 - <<'PY'
from pathlib import Path
import os
import pyautogui as ui
import time
ui.FAILSAFE=False
assert ui.size() == (1280,800)
ui.click(840,640) # Close the initial Start panel
ui.moveTo(253,326,duration=.15) # File tile Leggimi.txt
ui.dragTo(505,228,duration=.9,button='left') # Drop onto Progetti folder
p=Path(os.environ['DEMO'])
src=p/'Home'/'Leggimi.txt'
dst=p/'Home'/'Progetti'/'Leggimi.txt'
assert dst.is_file() and dst.read_bytes()==src.read_bytes(), 'GUI drag/drop copy unsuccessful'
assert src.is_file(), 'GUI drag/drop unexpectedly removed source'
ui.click(1012,771) # Open quick controls
ui.sleep(.25)
ui.click(990,471) # Turn Wi-Fi demo off; should not change host Wi-Fi
ui.sleep(.25)
import json
logs=[json.loads(x) for x in (p/'events.jsonl').read_text().splitlines()]
assert any(x['event']=='file-copy' and x['destination']=='Progetti' for x in logs), 'File copy UI log missing'
assert any(x['event']=='quick-toggle' and x['control']=='Wi-Fi' and x['enabled']==False for x in logs), 'Quick setting not interactive'
ui.screenshot().save(os.environ['SCREENSHOT'])
print('PASS: Xvfb 1280x800; real mouse drag/drop copy; source retained; quick-settings toggle; screenshot')
print('SCREENSHOT:',os.environ['SCREENSHOT'])
PY
if [[ -s "$D/gui.log" ]]; then cat "$D/gui.log"; fi
