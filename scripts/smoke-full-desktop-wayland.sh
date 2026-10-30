#!/usr/bin/env bash
# Real nested Wayland end-to-end smoke: desktop layer, panel, Files, Settings,
# IPC workspace switching, captured X11 host pixels, and clean shutdown.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
[[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]] || { echo "BLOCKED: requires a host display" >&2; exit 3; }
for name in melania-compositor melania-desktop-surface melania-panel melania-files melania-settings melania-winctl; do
  [[ -x "target/debug/$name" ]] || { echo "BLOCKED: missing $name" >&2; exit 3; }
done
mkdir -p reports
TEMP="$(mktemp -d -t melania-full-wayland-XXXXXX)"
mkdir -m 700 "$TEMP/runtime"
export XDG_RUNTIME_DIR="$TEMP/runtime"
PID=''
cleanup(){
  if [[ -n "$PID" ]]; then
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
  fi
  cp "$TEMP/compositor.log" reports/wayland-full-compositor.log 2>/dev/null || true
  cp "$TEMP/windows.log" reports/wayland-full-windows.log 2>/dev/null || true
  rm -rf "$TEMP"
}
trap cleanup EXIT
target/debug/melania-compositor \
  --command "$ROOT/target/debug/melania-desktop-surface" \
  --command "$ROOT/target/debug/melania-panel" \
  --command "$ROOT/target/debug/melania-files" \
  --command "$ROOT/target/debug/melania-settings" \
  >"$TEMP/compositor.log" 2>&1 &
PID=$!
CONTROL_SOCKET=''
for i in $(seq 1 100); do
  kill -0 "$PID" 2>/dev/null || { echo 'FAIL: compositor exited at startup' >&2; cat "$TEMP/compositor.log" >&2; exit 1; }
  CONTROL_SOCKET="$(find "$TEMP/runtime" -maxdepth 1 -type s -name 'melania-*control.sock' -print -quit)"
  [[ -z "$CONTROL_SOCKET" ]] || break
  sleep .1
done
[[ -n "$CONTROL_SOCKET" ]] || { echo 'FAIL: no control socket' >&2; cat "$TEMP/compositor.log" >&2; exit 1; }
export MELANIA_CONTROL_SOCKET="$CONTROL_SOCKET"
READY=0
for i in $(seq 1 120); do
  kill -0 "$PID" 2>/dev/null || { echo 'FAIL: compositor exited before windows were mapped' >&2; cat "$TEMP/compositor.log" >&2; exit 1; }
  if target/debug/melania-winctl list >"$TEMP/windows.log" 2>/dev/null; then
    if grep -Eiq '(melania[[:space:]._-]*files|org\.melaniadesktop\.Files)' "$TEMP/windows.log" && \
       grep -Eiq '(impostazioni|settings|org\.melaniadesktop\.Settings)' "$TEMP/windows.log"; then
       READY=1; break
    fi
  fi
  sleep .15
done
if [[ "$READY" != 1 ]]; then
  echo 'FAIL: Files and Settings did not both map' >&2
  cat "$TEMP/windows.log" >&2 || true; cat "$TEMP/compositor.log" >&2; exit 1
fi
for name in melania-desktop-surface melania-panel melania-files melania-settings; do
  pgrep -P "$PID" -f "target/debug/$name" >/dev/null || { echo "FAIL: $name exited early" >&2; cat "$TEMP/compositor.log" >&2; exit 1; }
done
[[ "$(target/debug/melania-winctl status)" == *'1'* ]] || { echo 'FAIL: wrong initial workspace' >&2; exit 1; }
target/debug/melania-winctl workspace 2 >/dev/null
[[ "$(target/debug/melania-winctl status)" == *'2'* ]] || { echo 'FAIL: workspace 2 not active' >&2; exit 1; }
target/debug/melania-winctl workspace 1 >/dev/null
[[ "$(target/debug/melania-winctl status)" == *'1'* ]] || { echo 'FAIL: workspace 1 not restored' >&2; exit 1; }
sleep 2
if command -v scrot >/dev/null; then
  scrot -z -o reports/nested-wayland-full-desktop.png
  python3 - <<'PY'
from PIL import Image
im = Image.open('reports/nested-wayland-full-desktop.png').convert('RGB')
assert im.width >= 1024 and im.height >= 600, im.size
sample = im.resize((128,80))
unique = len(set(sample.getdata()))
assert unique > 8, f'rendering seems blank, distinct colors: {unique}'
# A colorful wallpaper is not enough: a large bright GTK window must be visible.
bright = sum(1 for r,g,b in sample.getdata() if min(r,g,b) > 185)
assert bright > 300, f'GTK windows are hidden or not drawn (bright samples={bright})'
print(f'PASS: real Xvfb rendering {im.size}, {unique} colors, {bright} bright window samples')
PY
fi
kill -0 "$PID" || { echo 'FAIL: compositor stopped during test' >&2; exit 1; }
target/debug/melania-winctl quit >/dev/null
for i in $(seq 1 60); do
  if ! kill -0 "$PID" 2>/dev/null; then break; fi
  sleep .1
done
if kill -0 "$PID" 2>/dev/null; then echo 'FAIL: quit did not close compositor' >&2; exit 1; fi
wait "$PID" || { echo 'FAIL: compositor exited with error' >&2; exit 1; }
PID=''
echo 'PASS: desktop layer, panel, Files, Settings, workspace switch, screenshot and clean quit'
echo 'NOT TESTED: hardware DRM/seat, XWayland, HiDPI, drag-and-drop or production Aero.'
