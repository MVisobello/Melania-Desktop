#!/usr/bin/env bash
# Headless integration smoke test for the real nested Smithay compositor.
set -euo pipefail
export LIBGL_ALWAYS_SOFTWARE=1
export GALLIUM_DRIVER=llvmpipe
export WINIT_UNIX_BACKEND=x11
export XDG_RUNTIME_DIR="$(mktemp -d)"
chmod 700 "$XDG_RUNTIME_DIR"
log="${MELANIA_SMOKE_LOG:-melania-wayland-smoke.log}"
pid=""
cleanup() {
  if [[ -n "$pid" ]]; then kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; fi
  rm -rf "$XDG_RUNTIME_DIR"
}
trap cleanup EXIT
: >"$log"
timeout --signal=TERM 60s target/debug/melania-compositor >"$log" 2>&1 &
pid=$!
socket=""
for _ in $(seq 1 100); do
  if ! kill -0 "$pid" 2>/dev/null; then
    echo "FAIL: compositor exited before creating a Wayland socket"; cat "$log"; exit 1
  fi
  socket="$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name 'melania-*' ! -name '*control.sock' -printf '%f\n' | head -1)"
  if [[ -n "$socket" ]]; then break; fi
  sleep 0.1
done
if [[ -z "$socket" ]]; then
  echo "FAIL: Wayland socket not created"; cat "$log"; exit 1
fi
export WAYLAND_DISPLAY="$socket"
echo "Wayland socket ready: $WAYLAND_DISPLAY"
# Connect a REAL Wayland protocol client; inspect the advertised globals.
timeout 20s wayland-info >wayland-globals.log 2>&1 || {
  echo "FAIL: Wayland client cannot connect"; cat "$log"; cat wayland-globals.log; exit 1;
}
for protocol in wl_compositor wl_shm wl_seat xdg_wm_base; do
  if ! grep -q "$protocol" wayland-globals.log; then
    echo "FAIL: missing Wayland global: $protocol"; cat wayland-globals.log; exit 1
  fi
done
# Also inspect the compositor's separate, access-restricted control socket.
control="$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name '*control.sock' | head -1)"
[[ -n "$control" ]] || { echo 'FAIL: control socket missing'; exit 1; }
[[ "$(stat -c '%a' "$control")" == '600' ]] || { echo 'FAIL: insecure control socket mode'; exit 1; }
echo "PASS: nested compositor and Wayland globals are operational"
