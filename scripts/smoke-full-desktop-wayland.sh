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
# These dependencies are essential to prove actual pixels and keyboard input.
# A process-only smoke must never silently pass as a full desktop test.
for executable in scrot xdotool python3; do
  command -v "$executable" >/dev/null || {
    echo "BLOCKED: required full-desktop visual/input tool missing: $executable" >&2
    exit 3
  }
done
python3 -c 'from PIL import Image, ImageChops' || {
  echo 'BLOCKED: Pillow required for visual assertion and screenshot provenance' >&2
  exit 3
}
[[ -f tools/check_shared_window_chrome.py ]] || {
  echo 'BLOCKED: shared window chrome pixel validator missing' >&2
  exit 3
}
mkdir -p reports
# Ensure the XDG Desktop directory exists on fresh CI runners.
mkdir -p "$HOME/Desktop"
TEMP="$(mktemp -d -t melania-full-wayland-XXXXXX)"
mkdir -m 700 "$TEMP/runtime"
mkdir -m 700 "$TEMP/config"
export XDG_RUNTIME_DIR="$TEMP/runtime"
# No CI settings test may mutate developer or runner personal config files.
export XDG_CONFIG_HOME="$TEMP/config"
# Real GVfs local URI exercises the exact same GIO tab rendering pipeline as
# network:/// and sftp:/// without relying on a server on shared CI runners.
mkdir -p "$TEMP/gvfs-case"
printf '%s\n' 'real GTK/GIO/GVfs navigation' > "$TEMP/gvfs-case/gio-proof.txt"
export MELANIA_SMOKE_GIO_FILE_URI="file://$TEMP/gvfs-case"
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
# In this isolated test only, reveal Start before taking the true compositor
# screenshot so its icon sizes and real layout can be visually reviewed.
export MELANIA_PROFILE_FILES=1
export MELANIA_PROFILE_SETTINGS=1
export MELANIA_SMOKE_SETTINGS_LAZY=1
export MELANIA_SMOKE_SETTINGS_SERVICES=1
MELANIA_PROFILE_COMPOSITOR=1 MELANIA_SMOKE_FILES_LISTING=1 MELANIA_SMOKE_OPEN_START=1 MELANIA_SMOKE_OUTSIDE_CLICK=1 target/debug/melania-compositor \
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
# Assert the shell was actually rendered and submitted *before* any
# application window was spawned; process-start order alone isn't evidence.
python3 - "$TEMP/compositor.log" <<'PYBOOT'
from pathlib import Path
import re, sys
log=Path(sys.argv[1]).read_text().splitlines()
def line_index(pattern):
    for i,line in enumerate(log):
        if re.search(pattern,line): return i
    raise AssertionError(f"missing shell startup evidence: {pattern}")
start_d=line_index(r"MELANIA_CLIENT_STARTED program=.*melania-desktop-surface")
start_p=line_index(r"MELANIA_CLIENT_STARTED program=.*melania-panel")
paint=line_index(r"MELANIA_BOOT_SHELL_FRAME_MS=[0-9]+ background=[1-9][0-9]* panel=[1-9][0-9]*")
released=line_index(r"MELANIA_BOOT_APPLICATIONS_RELEASED_MS=[0-9]+ fallback=false")
started=[int(m.group(1)) for line in log
         if (m:=re.search(r"MELANIA_BOOT_APP_STARTED_MS=([0-9]+) program=",line))]
assert len(started)>=2, f"missing per-app startup evidence: {started}"
assert started[1]-started[0]>=150, f"GTK startup not staggered: {started}"
files=line_index(r"MELANIA_CLIENT_STARTED program=.*melania-files")
settings=line_index(r"MELANIA_CLIENT_STARTED program=.*melania-settings")
assert max(start_d,start_p)<paint<released<min(files,settings), (start_d,start_p,paint,released,files,settings)
print("PASS: real desktop wallpaper and panel rendered before Files/Settings launch")
PYBOOT
for name in melania-desktop-surface melania-panel melania-files melania-settings; do
  pgrep -P "$PID" -f "target/debug/$name" >/dev/null || { echo "FAIL: $name exited early" >&2; cat "$TEMP/compositor.log" >&2; exit 1; }
done
[[ "$(target/debug/melania-winctl status)" == *'1'* ]] || { echo 'FAIL: wrong initial workspace' >&2; exit 1; }
target/debug/melania-winctl workspace 2 >/dev/null
[[ "$(target/debug/melania-winctl status)" == *'2'* ]] || { echo 'FAIL: workspace 2 not active' >&2; exit 1; }
target/debug/melania-winctl workspace 1 >/dev/null
[[ "$(target/debug/melania-winctl status)" == *'1'* ]] || { echo 'FAIL: workspace 1 not restored' >&2; exit 1; }
# Do not mistake an unpainted GTK window for a working desktop. GPU/software
# initialization is asynchronous on CI. Wait for at least one real xdg buffer.
READY_RENDER=0
for i in $(seq 1 140); do
  if grep -Eq 'MELANIA_RENDER .*xdg=[1-9]' "$TEMP/compositor.log"; then
    READY_RENDER=1; break
  fi
  kill -0 "$PID" 2>/dev/null || { echo 'FAIL: compositor died while waiting for GTK buffers' >&2; exit 1; }
  sleep .25
done
if [[ "$READY_RENDER" != 1 ]]; then
  echo 'FAIL: GTK toplevels registered but no xdg surface rendered after 35 seconds' >&2
  grep 'MELANIA_RENDER' "$TEMP/compositor.log" | tail -10 >&2 || true
  exit 1
fi
# A mapped Files window alone does not prove that its asynchronous filesystem
# worker returned and populated the GTK model. Wait for the real completion.
FILES_LISTED=0
for i in $(seq 1 100); do
  if grep -q 'MELANIA_FILES_LISTING_READY count=' "$TEMP/compositor.log"; then
    FILES_LISTED=1; break
  fi
  kill -0 "$PID" 2>/dev/null || { echo 'FAIL: compositor exited before Files list finished' >&2;exit 1; }
  sleep .1
done
[[ "$FILES_LISTED" == 1 ]] || { echo 'FAIL: no async Melania Files list completion';exit 1; }
echo 'PASS: asynchronous filesystem listing reached real GTK Files window'
# Settings must first present its window, then construct real XFCE panel
# controls on explicit navigation. The marker is emitted after fill.
SETTINGS_LAZY_READY=0
for i in $(seq 1 100); do
  if grep -Eq 'MELANIA_SETTINGS_PAGE_BUILD_MS=[0-9]+ page=xfce-panel' "$TEMP/compositor.log"; then
    SETTINGS_LAZY_READY=1; break
  fi
  sleep .1
done
[[ "$SETTINGS_LAZY_READY" == 1 ]] || {
  echo 'FAIL: real Settings lazy functional category was not materialized' >&2
  grep 'MELANIA_SETTINGS_' "$TEMP/compositor.log" >&2 || true
  exit 1
}
grep -q 'MELANIA_SETTINGS_FIRST_WINDOW_MS=' "$TEMP/compositor.log" || {
  echo 'FAIL: Settings first window evidence missing' >&2; exit 1
}
echo 'PASS: Settings first window appeared before deferred XFCE panel controls were built'
# Exercise real background NetworkManager and BlueZ service probes. They
# may report absent controllers in CI; neither should hold GTK's event loop.
SERVICE_READY=0
for i in $(seq 1 110); do
  if grep -q 'MELANIA_SETTINGS_ASYNC_NAVIGATION_OK' "$TEMP/compositor.log" && \
     grep -Eq 'MELANIA_SETTINGS_SERVICE_MS=[0-9]+ service=network-status outcome=' "$TEMP/compositor.log" && \
     grep -Eq 'MELANIA_SETTINGS_SERVICE_MS=[0-9]+ service=wifi-detect outcome=' "$TEMP/compositor.log" && \
     grep -Eq 'MELANIA_SETTINGS_SERVICE_MS=[0-9]+ service=bluetooth-detect outcome=' "$TEMP/compositor.log"; then
    SERVICE_READY=1; break
  fi
  sleep .1
done
[[ "$SERVICE_READY" == 1 ]] || {
  echo 'FAIL: Settings background network/Bluetooth probes did not finish or time out' >&2
  grep 'MELANIA_SETTINGS_' "$TEMP/compositor.log" >&2 || true
  exit 1
}
echo 'PASS: Settings navigated Network/Bluetooth and received bounded background probes'
# The actual GTK Files window should be submitted before the expensive
# first browser tab, which in turn precedes the asynchronous GIO listing.
python3 - "$TEMP/compositor.log" <<'PYFILESFIRST'
from pathlib import Path
import sys
lines=Path(sys.argv[1]).read_text().splitlines()
def first(needle):
    for i,line in enumerate(lines):
        if needle in line:return i
    raise AssertionError(f"Missing startup marker: {needle}")
window=first("MELANIA_FILES_FIRST_WINDOW_MS=")
tab=first("MELANIA_FILES_FIRST_TAB_MS=")
listing=first("MELANIA_FILES_LISTING_READY count=")
assert window<tab<listing,(window,tab,listing)
print("PASS: Files submitted its window before building its first tab and completing GIO listing")
PYFILESFIRST
# A quiet frame must not continually redraw the whole software-rendered
# desktop. Verify at least one real idle skip was counted by the compositor.
IDLE_SKIP=0
for i in $(seq 1 60); do
  if grep -Eq 'MELANIA_COMPOSITOR_FPS=.* idle_skipped=[1-9][0-9]*' "$TEMP/compositor.log"; then
    IDLE_SKIP=1; break
  fi
  sleep .1
done
[[ "$IDLE_SKIP" == 1 ]] || {
  echo 'FAIL: compositor did not skip unchanged frames' >&2
  grep 'MELANIA_COMPOSITOR_FPS' "$TEMP/compositor.log" >&2 || true
  exit 1
}
echo 'PASS: real nested compositor skipped unchanged EGL redraws'

# Profile the real GIO-backed folder scan and GTK render. The CI session uses
# LIBGL_ALWAYS_SOFTWARE=1, so thumbnail budget should be 16, not 48.
grep -Eq 'MELANIA_FILES_SCAN_MS=[0-9]+' "$TEMP/compositor.log" || {
  echo 'FAIL: missing Files directory scan timing' >&2; exit 1;
}
grep -Eq 'MELANIA_FILES_RENDER_MS=[0-9]+ entries=[0-9]+ thumbnails_queued=[0-9]+ software_budget=16' "$TEMP/compositor.log" || {
  echo 'FAIL: missing Files UI render timing or software thumbnail cap' >&2; exit 1;
}
echo 'PASS: real Files scan and GTK rendering profiled with software thumbnail budget 16'
# Ensure remote-mode GIO tabs are not just source code: the worker must
# enumerate a real file URI and GTK must populate its rows.
GIO_LISTED=0
for i in $(seq 1 100); do
  if grep -q 'MELANIA_GIO_LISTING_READY count=1' "$TEMP/compositor.log"; then
    GIO_LISTED=1; break
  fi
  kill -0 "$PID" 2>/dev/null || { echo 'FAIL: compositor exited during GIO tab scan';exit 1; }
  sleep .1
done
[[ "$GIO_LISTED" == 1 ]] || { echo 'FAIL: GVfs file:// tab never listed real file';exit 1; }
echo 'PASS: real GTK network-style GIO URI tab enumerated a local file'
sleep .8
# Request a real compositor-owned window-only thumbnail through the same
# 0600 control socket used by the taskbar. Validate the actual BMP pixels.
FILES_ID="$(awk -F '\t' 'tolower($3) ~ /org.melaniadesktop.files/ || tolower($4) ~ /melania files/ {print $1;exit}' "$TEMP/windows.log")"
[[ "$FILES_ID" =~ ^[0-9]+$ ]] || { echo 'FAIL: no Files window id for preview' >&2;exit 1; }
THUMBNAIL="$(target/debug/melania-winctl preview "$FILES_ID")" || {
  echo 'FAIL: per-window offscreen GPU preview unavailable' >&2; exit 1;
}
python3 - "$THUMBNAIL" <<'PYTHUMB'
from PIL import Image
from pathlib import Path
import os, stat, sys
p=Path(sys.argv[1])
assert p.is_file() and p.suffix=='.bmp', p
assert stat.S_IMODE(p.stat().st_mode)==0o600, oct(p.stat().st_mode)
assert stat.S_IMODE(p.parent.stat().st_mode)==0o700, oct(p.parent.stat().st_mode)
image=Image.open(p).convert('RGB')
assert image.size==(256,160), image.size
colors=len(set(image.resize((64,40)).getdata()))
assert colors>4, f'thumbnail is blank or unpainted: {colors} colors'
print(f'PASS: private window-only GPU thumbnail {image.size}, {colors} sampled colors')
PYTHUMB
# Client startup is asynchronous: whichever toplevel mapped last may have
# focus. Explicitly raise Settings to show BOTH window chrome controls in
# the same genuine screenshot, rather than trusting window mapping order.
SETTINGS_ID="$(awk -F '\t' 'tolower($3) ~ /org.melaniadesktop.settings/ || tolower($4) ~ /impostazioni|settings/ {print $1;exit}' "$TEMP/windows.log")"
[[ "$SETTINGS_ID" =~ ^[0-9]+$ ]] || {
  echo 'FAIL: no Settings window id available for chrome comparison' >&2; exit 1;
}
target/debug/melania-winctl activate "$SETTINGS_ID" >/dev/null || {
  echo 'FAIL: cannot activate Settings window for chrome QA' >&2; exit 1;
}
sleep .5
# Synthetic CI desktop only: preserve one sample for visual QA.
cp "$THUMBNAIL" reports/nested-wayland-file-preview.bmp
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
# The menu is intentionally opened in CI. Its top edge must not be clipped
# against the screen while the list scrolls inside the constrained popup.
top=im.getpixel((20,20))
menu=im.getpixel((20,500))
difference=sum(abs(x-y) for x,y in zip(top,menu))
assert difference > 25, f'Start menu covers the top of the display; must scroll apps internally ({top=}, {menu=})'
print(f'PASS: real Xvfb rendering {im.size}, {unique} colors, {bright} bright window samples, Start top clearance {difference}')
PY
  # Both the foreground Settings window and background Files window must
  # display identical Win7 controls in the actual composited screenshot.
  python3 tools/check_shared_window_chrome.py reports/nested-wayland-full-desktop.png
fi
# The first screenshot intentionally shows Start at launch. Now exercise the
# REAL keyboard path through the nested X11 host: Super closes and reopens it,
# then ordinary typing must go straight into the focused Start search entry.
# Never inject host-wide keys outside isolated xvfb-run.
if command -v xdotool >/dev/null; then
  HOST_WINDOW="$(xdotool search --onlyvisible --pid "$PID" 2>/dev/null | head -n1 || true)"
  if [[ -z "$HOST_WINDOW" ]]; then
    HOST_WINDOW="$(xdotool search --onlyvisible --name . 2>/dev/null | head -n1 || true)"
  fi
  [[ -n "$HOST_WINDOW" ]] || { echo 'FAIL: no Xvfb Melania compositor window for keyboard QA' >&2; exit 1; }
  xdotool windowfocus --sync "$HOST_WINDOW"
  xdotool key --clearmodifiers Super_L
  for i in $(seq 1 35); do
    grep -q 'MELANIA_START_IPC toggled open=false' "$TEMP/compositor.log" && break
    sleep .1
  done
  grep -q 'MELANIA_START_IPC toggled open=false' "$TEMP/compositor.log" || {
    echo 'FAIL: Super did not close Start' >&2
    tail -70 "$TEMP/compositor.log" >&2; exit 1;
  }
  xdotool key --clearmodifiers Super_L
  for i in $(seq 1 35); do
    grep -q 'MELANIA_START_IPC toggled open=true' "$TEMP/compositor.log" && break
    sleep .1
  done
  grep -q 'MELANIA_START_IPC toggled open=true' "$TEMP/compositor.log" || {
    echo 'FAIL: Super did not reopen Start' >&2
    tail -70 "$TEMP/compositor.log" >&2; exit 1;
  }
  xdotool type --clearmodifiers --delay 50 'melaniasearch'
  for i in $(seq 1 35); do
    grep -q 'MELANIA_START_SEARCH_RECEIVED' "$TEMP/compositor.log" && break
    sleep .1
  done
  grep -q 'MELANIA_START_SEARCH_RECEIVED' "$TEMP/compositor.log" || {
    echo 'FAIL: typed keys did not reach the Start search field' >&2
    tail -70 "$TEMP/compositor.log" >&2; exit 1;
  }
  if command -v scrot >/dev/null; then
    sleep .25
    scrot -z -o reports/nested-wayland-start-search.png
  fi
  echo 'PASS: lone Super toggles Start closed/open, text reaches search without mouse click'
fi
# Publish a copy of the ACTUAL running Melania screenshot with provenance.
# Source pixels are never redrawn or synthesized. A second screenshot proves
# the Start search visibly changed after the real keyboard injection.
if command -v scrot >/dev/null && command -v xdotool >/dev/null; then
  python3 scripts/check-real-desktop-preview.py \
    --screenshot reports/nested-wayland-full-desktop.png \
    --search reports/nested-wayland-start-search.png \
    --windows "$TEMP/windows.log" \
    --output reports/melania-real-desktop-preview.png \
    --manifest reports/melania-real-desktop-preview.json
fi
# This evidence is required, not an optional by-product of a passing test.
# Screenshots and machine-readable provenance must be available for Alpha QA.
for proof in reports/nested-wayland-full-desktop.png \
             reports/nested-wayland-start-search.png \
             reports/melania-real-desktop-preview.png \
             reports/melania-real-desktop-preview.json; do
  [[ -s "$proof" ]] || { echo "FAIL: missing mandatory visual evidence: $proof" >&2; exit 1; }
done
python3 - <<'PYEVIDENCE'
import json
from pathlib import Path
data = json.loads(Path('reports/melania-real-desktop-preview.json').read_text())
assert data['capture_type'] == 'real_nested_wayland_xvfb', data
assert data['native_kms_desktop'] is False, data
assert data['synthetic_ui_pixels'] is False, data
assert data['files_window_mapped'] and data['settings_window_mapped'], data
assert data['start_search_visual_change'], data
print('PASS: mandatory real nested screenshot and Start input evidence retained')
PYEVIDENCE
# Prove a real mouse press outside Start reaches the nested compositor and
# dismisses every transient overlay without disconnecting the GTK panel.
# Both the before and after frames come from the same actual Xvfb session.
xdotool mousemove --window "$HOST_WINDOW" 920 55 click 1
DISMISSED=0
for i in $(seq 1 35); do
  if grep -q 'MELANIA_OUTSIDE_CLICK_DISMISSED previously_open=true now_open=false' "$TEMP/compositor.log"; then
    DISMISSED=1; break
  fi
  kill -0 "$PID" 2>/dev/null || { echo 'FAIL: desktop crashed after outside-click' >&2; exit 1; }
  sleep .1
done
[[ "$DISMISSED" == 1 ]] || {
  echo 'FAIL: desktop click did not close the visible Start popup' >&2
  tail -80 "$TEMP/compositor.log" >&2; exit 1;
}
pgrep -P "$PID" -f 'target/debug/melania-panel' >/dev/null || {
  echo 'FAIL: outside-click disconnected taskbar client' >&2; exit 1;
}
sleep .35
scrot -z -o reports/nested-wayland-start-dismissed.png
python3 - <<'PYDISMISS'
from PIL import Image, ImageChops
before=Image.open('reports/nested-wayland-start-search.png').convert('RGB')
after=Image.open('reports/nested-wayland-start-dismissed.png').convert('RGB')
assert before.size==after.size
region=(12,320,min(390,before.width-1),min(760,before.height-1))
diff=ImageChops.difference(before.crop(region),after.crop(region))
changed=sum(1 for pixel in diff.getdata() if max(pixel)>35)
assert changed>1000, f'Start was not visibly dismissed: changed pixels={changed}'
print(f'PASS: real outside mouse click concealed Start and kept panel alive ({changed} changed pixels)')
PYDISMISS
# Verify a REAL running Melania taskbar accepts a changed user config and
# moves from the bottom to the top edge without a restart or compositor exit.
# The setting is isolated under the temporary CI XDG_CONFIG_HOME.
mkdir -p "$XDG_CONFIG_HOME/melania"
cat > "$XDG_CONFIG_HOME/melania/settings.conf" <<'EOPANEL'
panel_height=72
panel_position=top
panel_auto_hide=false
panel_opacity=100
EOPANEL
sleep 2
kill -0 "$PID" 2>/dev/null || {
  echo 'FAIL: compositor exited after live panel configuration change' >&2; exit 1;
}
pgrep -P "$PID" -f 'target/debug/melania-panel' >/dev/null || {
  echo 'FAIL: panel exited after live configuration change' >&2; exit 1;
}
if command -v scrot >/dev/null; then
  scrot -z -o reports/nested-wayland-panel-top.png
  python3 - <<'PYPANEL'
from pathlib import Path
from PIL import Image
before=Image.open("reports/nested-wayland-full-desktop.png").convert("RGB")
after=Image.open("reports/nested-wayland-panel-top.png").convert("RGB")
assert before.size==after.size
# The top 8px used to be wallpaper. Now an opaque panel spans the upper
# edge, while the UI's host window and nested compositor remain alive.
xs=(before.width//2, before.width*2//3, before.width*5//6)
diffs=[sum(abs(a-b) for a,b in zip(before.getpixel((x,8)),after.getpixel((x,8)))) for x in xs]
assert max(diffs)>20, f"panel did not visually move to the upper edge: {diffs}"
print("PASS: actual running panel reanchored to top edge with persisted preferences",diffs)
PYPANEL
fi
# Change the *real* wallpaper while GTK Desktop is running. The color is
# deliberately synthetic and remains inside the isolated CI temporary tree.
python3 - "$TEMP/xfce-wallpaper-test.png" <<'PYWALL'
from PIL import Image
import sys
Image.new("RGB",(1280,800),(19,161,82)).save(sys.argv[1])
PYWALL
cat >> "$XDG_CONFIG_HOME/melania/settings.conf" <<EOF
desktop_wallpaper=$TEMP/xfce-wallpaper-test.png
desktop_wallpaper_fit=cover
desktop_show_icons=false
EOF
sleep 2
pgrep -P "$PID" -f 'target/debug/melania-desktop-surface' >/dev/null || {
  echo 'FAIL: desktop surface crashed on wallpaper update' >&2;exit 1;
}
if command -v scrot >/dev/null; then
  scrot -z -o reports/nested-wayland-wallpaper-live.png
  python3 - <<'PYWALLCHECK'
from PIL import Image
before=Image.open("reports/nested-wayland-full-desktop.png").convert("RGB")
after=Image.open("reports/nested-wayland-wallpaper-live.png").convert("RGB")
# Leftmost background strip is outside regular application windows.
points=[(4,100),(8,150),(12,190)]
diffs=[sum(abs(a-b) for a,b in zip(before.getpixel(p),after.getpixel(p))) for p in points]
assert max(diffs)>60, f"wallpaper did not update in live desktop layer: {diffs}"
print("PASS: actual GTK desktop wallpaper changed live from XDG preferences",diffs)
PYWALLCHECK
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
