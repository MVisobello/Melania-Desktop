#!/usr/bin/env bash
# Real nested software entrypoint smoke test under an isolated Xvfb display.
# No host VT, DRM ownership, native login or XWayland acceptance is implied.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
[[ -n "$(printenv DISPLAY 2>/dev/null)" ]] || { echo 'BLOCKED: isolated Xvfb host DISPLAY required' >&2; exit 78; }
for tool in setsid env grep mktemp; do
  command -v "$tool" >/dev/null || { echo "MISSING_DEPENDENCY: $tool" >&2; exit 3; }
done
for bin in melania-compositor melania-panel melania-desktop-surface; do
  [[ -x "$ROOT/target/debug/$bin" ]] || { echo "BLOCKED: build $bin first" >&2; exit 3; }
done
mkdir -p reports
TEMP="$(mktemp -d -t melania-software-entrypoint-XXXXXX)"
mkdir -m 700 "$TEMP/runtime" "$TEMP/config"
export XDG_RUNTIME_DIR="$TEMP/runtime"
export XDG_CONFIG_HOME="$TEMP/config"
SESSION_PID=''
cleanup() {
  trap - EXIT INT TERM
  if [[ -n "$SESSION_PID" ]]; then
    # The new session/process group encloses dbus-run-session, compositor,
    # desktop, panel and notification daemon. No surviving helper processes.
    kill -TERM -- "-$SESSION_PID" 2>/dev/null || true
    sleep .3
    kill -KILL -- "-$SESSION_PID" 2>/dev/null || true
    wait "$SESSION_PID" 2>/dev/null || true
  fi
  cp "$TEMP/session.log" reports/nested-software-entrypoint.log 2>/dev/null || true
  rm -rf "$TEMP"
}
trap cleanup EXIT INT TERM
# Do not pass GSK_RENDERER/LIBGL_ALWAYS_SOFTWARE from the test harness:
# the actual --nested-software launcher must enable them itself.
setsid env -u GSK_RENDERER -u LIBGL_ALWAYS_SOFTWARE \
  MELANIA_PREVIEW_OPEN_APPS=0 bash scripts/melania-session --nested-software \
  >"$TEMP/session.log" 2>&1 &
SESSION_PID=$!
ready=0
for ((attempt=0;attempt<180;attempt++)); do
  if grep -Fq 'MELANIA_NESTED_SOFTWARE_RENDERER: GSK_RENDERER=cairo LIBGL_ALWAYS_SOFTWARE=1' "$TEMP/session.log" &&
     grep -Fq 'NESTED_PREFLIGHT_OK:' "$TEMP/session.log" &&
     grep -Eq 'MELANIA_CLIENT_STARTED program=.*melania-desktop-surface' "$TEMP/session.log" &&
     grep -Eq 'MELANIA_CLIENT_STARTED program=.*melania-panel' "$TEMP/session.log" &&
     grep -Eq 'MELANIA_BOOT_SHELL_FRAME_MS=[0-9]+ background=[1-9][0-9]* panel=[1-9][0-9]*' "$TEMP/session.log" &&
     grep -Fq 'MELANIA_PANEL_STAGE=taskbar_presented' "$TEMP/session.log"; then
    # A stack that died immediately after printing markers is not a pass.
    if kill -0 "$SESSION_PID" 2>/dev/null; then
      ready=1
      break
    fi
  fi
  if ! kill -0 "$SESSION_PID" 2>/dev/null; then
    echo 'FAIL: software nested session exited before desktop and panel rendered' >&2
    tail -n 100 "$TEMP/session.log" >&2 || true
    exit 1
  fi
  sleep .5
done
if [[ "$ready" != 1 ]]; then
  echo 'FAIL: software nested entrypoint never rendered the real desktop+taskbar' >&2
  tail -n 100 "$TEMP/session.log" >&2 || true
  exit 1
fi
echo 'MELANIA_NESTED_SOFTWARE_REAL_DESKTOP_AND_PANEL_PASS=true'
echo 'IMPORTANT: Xvfb nested preview is NOT a physical GPU, DRM/VT or native-login acceptance test.'
