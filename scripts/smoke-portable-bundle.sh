#!/usr/bin/env bash
# Smoke the *extracted package* under an isolated Xvfb host session.
# This is a nested app test only. No VT, DRM acquisition or login-manager writes.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
# GitHub runners cache target/, so smoke the fresh archive generated in RUNNER_TEMP.
BUNDLE="${MELANIA_BUNDLE_TEST_ROOT:-$ROOT/target/melania-preview-release/melania-nested-preview}"
[[ -n "${DISPLAY:-}" ]] || { echo 'BLOCKED: bundle smoke requires isolated Xvfb DISPLAY' >&2; exit 3; }
[[ -x "$BUNDLE/run.sh" ]] || { echo 'BLOCKED: no packaged launcher' >&2; exit 3; }
command -v scrot >/dev/null || { echo 'BLOCKED: scrot required for actual pixel capture' >&2;exit 3; }

TMP="$(mktemp -d -t melania-preview-bundle-XXXXXX)"
mkdir -m 700 "$TMP/runtime" "$TMP/config"
mkdir -p "$TMP/home/Desktop"
export HOME="$TMP/home"
export XDG_RUNTIME_DIR="$TMP/runtime"
export XDG_CONFIG_HOME="$TMP/config"
export GDK_BACKEND=wayland
export LIBGL_ALWAYS_SOFTWARE=1
export GALLIUM_DRIVER=llvmpipe
export MELANIA_DEBUG_RENDER=1
export MELANIA_SMOKE_FILES_LISTING=1
export MELANIA_SMOKE_OPEN_START=1
export MELANIA_SMOKE_NOTIFICATION_SWITCH=1
PID=''
cleanup() {
  if [[ -n "$PID" ]]; then
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
  fi
  cp "$TMP/bundle.log" reports/bundle-nested-desktop.log 2>/dev/null || true
  rm -rf -- "$TMP"
}
trap cleanup EXIT
mkdir -p reports
"$BUNDLE/run.sh" --check >"$TMP/check.log" 2>&1 || {
  cat "$TMP/check.log" >&2; exit 1;
}
grep -q 'MELANIA_NESTED_BUNDLE_PREFLIGHT_OK=true' "$TMP/check.log"
# Exercise the same renderer fallback VirtualBox users will select. Keep
# the normal host path in the generic Ubuntu smoke as a separate gate.
if [[ "${MELANIA_BUNDLE_SOFTWARE_SMOKE:-0}" == 1 ]]; then
  "$BUNDLE/run.sh" --software >"$TMP/bundle.log" 2>&1 &
else
  "$BUNDLE/run.sh" >"$TMP/bundle.log" 2>&1 &
fi
PID=$!
if [[ "${MELANIA_BUNDLE_SOFTWARE_SMOKE:-0}" == 1 ]]; then
  for i in $(seq 1 60); do
    grep -q 'MELANIA_RENDERING_BACKEND=software' "$TMP/bundle.log" && break
    kill -0 "$PID" 2>/dev/null || break
    sleep .1
  done
  grep -q 'MELANIA_RENDERING_BACKEND=software' "$TMP/bundle.log" || {
    echo 'FAIL: VirtualBox software graphics fallback was not activated' >&2
    cat "$TMP/bundle.log" >&2
    exit 1
  }
fi

CONTROL_SOCKET=''
for i in $(seq 1 120); do
  kill -0 "$PID" 2>/dev/null || {
    echo 'FAIL: bundled compositor quit before control socket' >&2
    cat "$TMP/bundle.log" >&2; exit 1
  }
  CONTROL_SOCKET="$(find "$TMP/runtime" -maxdepth 1 -type s -name 'melania-*control.sock' -print -quit)"
  [[ -z "$CONTROL_SOCKET" ]] || break
  sleep .15
done
[[ -n "$CONTROL_SOCKET" ]] || { echo 'FAIL: no Wayland preview control socket' >&2;exit 1; }
export MELANIA_CONTROL_SOCKET="$CONTROL_SOCKET"
READY=0
for i in $(seq 1 160); do
  if "$BUNDLE/bin/melania-winctl" list >"$TMP/windows.log" 2>/dev/null && \
     grep -Eiq '(melania[[:space:]._-]*files|org\.melaniadesktop\.Files)' "$TMP/windows.log" && \
     grep -Eiq '(impostazioni|settings|org\.melaniadesktop\.Settings)' "$TMP/windows.log"; then
    READY=1
    break
  fi
  kill -0 "$PID" 2>/dev/null || { echo 'FAIL: bundle exited before GTK windows mapped' >&2;exit 1; }
  sleep .15
done
[[ "$READY" == 1 ]] || {
  echo 'FAIL: bundled Files/Settings windows did not both map' >&2
  cat "$TMP/bundle.log" >&2; exit 1
}
RENDERED=0
for i in $(seq 1 140); do
  if grep -Eq 'MELANIA_RENDER .*xdg=[1-9]' "$TMP/bundle.log"; then
    RENDERED=1;break
  fi
  sleep .25
done
[[ "$RENDERED" == 1 ]] || { echo 'FAIL: no GTK window submitted a real Wayland buffer' >&2;exit 1; }
# A toplevel-only screenshot can look like "just Melania Files over a
# background". Assert an actual painted top/overlay layer from the panel.
TOP_LAYER_RENDERED=0
for i in $(seq 1 80); do
  if grep -Eq 'MELANIA_RENDER .*top=[1-9][0-9]*' "$TMP/bundle.log"; then
    TOP_LAYER_RENDERED=1
    break
  fi
  kill -0 "$PID" 2>/dev/null || break
  sleep .25
done
[[ "$TOP_LAYER_RENDERED" == 1 ]] || {
  echo 'FAIL: top/overlay panel layer never rendered (Files alone is not a desktop)' >&2
  grep -E 'MELANIA_RENDER|MELANIA_CLIENT_|GSK|EGL|MESA' "$TMP/bundle.log" >&2 || true
  exit 1
}
echo 'PASS: real taskbar top-layer pixels submitted by nested Wayland compositor'
if ! grep -q 'MELANIA_PANEL_ICON_THEME_SELECTED=Melania-Preview' "$TMP/bundle.log"; then
  echo 'FAIL: portable panel did not load its own bundled icon theme' >&2
  grep -E 'ICON_THEME|melania-panel|icon-theme' "$TMP/bundle.log" >&2 || true
  exit 1
fi
echo 'PASS: panel is using its bundled original Melania icon theme'
# Real GTK signal: open quick controls then activate the notifications button,
# the same sequence which disconnected the whole taskbar in VirtualBox.
SWITCHED=0
for i in $(seq 1 140); do
  if grep -q 'MELANIA_SMOKE_NOTIFICATION_CLICK_OK' "$TMP/bundle.log"; then
    SWITCHED=1
    break
  fi
  kill -0 "$PID" 2>/dev/null || break
  sleep .2
done
[[ "$SWITCHED" == 1 ]] || {
  echo 'FAIL: notification click regression sequence did not complete' >&2
  tail -100 "$TMP/bundle.log" >&2
  exit 1
}
# The notification click must not destroy/re-create a Wayland role or kill
# the client owning the taskbar. A genuine compositor layer must stay mapped.
sleep .4
if grep -Eq 'MELANIA_CLIENT_EXIT program=.*melania-panel' "$TMP/bundle.log"; then
  echo 'FAIL: taskbar exited when switching from quick controls to notifications' >&2
  grep 'MELANIA_CLIENT_EXIT' "$TMP/bundle.log" >&2
  exit 1
fi
echo 'PASS: quick controls to notifications click preserves the panel client'
grep -Eq 'MELANIA_CLIENT_STARTED program=.*melania-panel' "$TMP/bundle.log" || {
  echo 'FAIL: bundled panel was never started' >&2
  exit 1
}
if grep -Eq 'MELANIA_CLIENT_EXIT program=.*melania-panel' "$TMP/bundle.log"; then
  echo 'FAIL: bundled panel exited while the desktop was starting' >&2
  grep 'MELANIA_CLIENT_EXIT' "$TMP/bundle.log" >&2
  exit 1
fi

sleep .8
scrot -z -o reports/bundle-nested-desktop.png
python3 - <<'PYPIXELS'
from PIL import Image
im=Image.open('reports/bundle-nested-desktop.png').convert('RGB')
assert im.width>=1024 and im.height>=600, im.size
sample=im.resize((128,80))
colors=len(set(sample.getdata()))
assert colors>8, f'blank bundled preview: {colors} colors'
print(f'PASS: bundled nested desktop actual pixels: {im.size}, {colors} colors')
PYPIXELS
"$BUNDLE/bin/melania-winctl" quit >/dev/null
for i in $(seq 1 70); do
  kill -0 "$PID" 2>/dev/null || break
  sleep .1
done
if kill -0 "$PID" 2>/dev/null; then echo 'FAIL: bundle did not quit cleanly' >&2;exit 1;fi
wait "$PID" || { echo 'FAIL: bundled launcher returned nonzero status' >&2;exit 1; }
PID=''
echo 'PASS: extracted portable bundle launched real nested GTK desktop and exited cleanly'
echo 'MELANIA_NATIVE_LOGIN_SESSION_READY=false'
