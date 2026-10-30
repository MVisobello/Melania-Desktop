#!/usr/bin/env bash
# Actual Wayland compositor smoke test on an X11/Wayland host (e.g. Xvfb).
# Leaves all host session settings untouched. FAILS if compositor exits.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if [[ -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]]; then
  echo 'BLOCKED: run this from an X11 or Wayland session (CI uses xvfb-run).' >&2
  exit 3
fi
if [[ ! -x target/debug/melania-compositor || ! -x target/debug/melania-winctl || ! -x target/debug/melania-files ]]; then
  echo 'BLOCKED: build melania-compositor, melania-winctl and melania-files before smoke test.' >&2
  exit 3
fi
# A listening socket with no actual Wayland client is NOT a usable desktop.
# Check a real GTK client, not merely a live process or IPC response.

TEMP="$(mktemp -d -t melania-wayland-XXXXXX)"
mkdir -m 700 "$TEMP/runtime"
export XDG_RUNTIME_DIR="$TEMP/runtime"
PID=''
cleanup(){
  if [[ -n "$PID" ]]; then
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
  fi
  if [[ -n "${KEEP_MELANIA_LOGS:-}" ]]; then
    cp -v "$TEMP/compositor.log" reports/wayland-smoke.log || true
  fi
  rm -rf "$TEMP"
}
trap cleanup EXIT
target/debug/melania-compositor --command "$ROOT/target/debug/melania-files" >"$TEMP/compositor.log" 2>&1 &
PID=$!
WAYLAND_SOCKET=''
for i in $(seq 1 80); do
  if ! kill -0 "$PID" 2>/dev/null; then
    echo 'FAIL: compositor terminated unexpectedly' >&2
    cat "$TEMP/compositor.log" >&2
    exit 1
  fi
  WAYLAND_SOCKET="$(find "$TEMP/runtime" -maxdepth 1 -type s -name 'melania-*' ! -name '*control.sock' -printf '%f\n' | head -1)"
  [[ -n "$WAYLAND_SOCKET" ]] && break
  sleep .1
done
if [[ -z "$WAYLAND_SOCKET" ]]; then
  echo 'FAIL: Wayland protocol socket not created' >&2
  cat "$TEMP/compositor.log" >&2
  exit 1
fi
CONTROL_SOCKET="$(find "$TEMP/runtime" -maxdepth 1 -type s -name 'melania-*control.sock' -print -quit)"
if [[ -z "$CONTROL_SOCKET" ]]; then
  echo 'FAIL: control socket not created' >&2; exit 1
fi
export MELANIA_CONTROL_SOCKET="$CONTROL_SOCKET"
# Verify that the compositor actually processes IPC events in the frame loop.
for i in $(seq 1 40); do
  if target/debug/melania-winctl list >"$TEMP/windows.log" 2>"$TEMP/ctl-error.log"; then break; fi
  sleep .2
done
if ! target/debug/melania-winctl list >"$TEMP/windows.log" 2>"$TEMP/ctl-error.log"; then
  echo 'FAIL: compositor does not respond to real IPC request' >&2
  cat "$TEMP/ctl-error.log" >&2
  exit 1
fi
# Require a toplevel mapped by the real Wayland GTK client.
# The compositor's window list is a stronger integration check than a socket.
CLIENT_PRESENT=0
for i in $(seq 1 80); do
  if ! kill -0 "$PID" 2>/dev/null; then
    echo 'FAIL: compositor stopped before GTK client mapped' >&2
    cat "$TEMP/compositor.log" >&2
    exit 1
  fi
  if target/debug/melania-winctl list >"$TEMP/windows.log" 2>"$TEMP/ctl-error.log"; then
    if grep -Eiq '(melania[[:space:]._-]*files|org\.melaniadesktop\.Files)' "$TEMP/windows.log"; then
      CLIENT_PRESENT=1
      break
    fi
  fi
  sleep .125
done
if (( ! CLIENT_PRESENT )); then
  echo 'FAIL: GTK client did not map a Melania Files toplevel within 10 seconds' >&2
  echo 'Window list:' >&2; cat "$TEMP/windows.log" >&2 || true
  echo 'Compositor log:' >&2; cat "$TEMP/compositor.log" >&2
  exit 1
fi
sleep .3
kill -0 "$PID" || { echo 'FAIL: compositor stopped during smoke test' >&2; cat "$TEMP/compositor.log" >&2; exit 1; }
echo "PASS: live Wayland socket $WAYLAND_SOCKET, IPC responsive, GTK toplevel registered"
echo 'IMPORTANT: pixel rendering, drag/drop and XWayland are NOT validated by this smoke test.' 
