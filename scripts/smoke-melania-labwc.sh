#!/usr/bin/env bash
# Real Wayland shell smoke under a disposable Xvfb-hosted labwc.
# This is a NESTED integration test, not proof of bare-metal DRM/VT login.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$ROOT"
for tool in labwc wlrctl xvfb-run dbus-run-session python3 scrot; do
  command -v "$tool" >/dev/null || { echo "MELANIA_LABWC_SMOKE_BLOCKED=missing_$tool" >&2; exit 3; }
done
for bin in melania-panel melania-desktop-surface melania-files melania-settings melania-notifyd; do
  [[ -x "$ROOT/target/debug/$bin" ]] ||
    { echo "MELANIA_LABWC_SMOKE_BLOCKED=missing_$bin" >&2; exit 3; }
done
bash "$ROOT/scripts/melania-labwc-session" --stage >/dev/null

temp="$(mktemp -d -t melania-labwc-ci-XXXXXX)"
mkdir -m 700 "$temp/runtime" "$temp/config" "$temp/data"
export XDG_RUNTIME_DIR="$temp/runtime"
export XDG_CONFIG_HOME="$temp/config"
export XDG_DATA_HOME="$temp/data"
export MELANIA_BIN_DIR="$ROOT/target/debug"
export MELANIA_LABWC_SMOKE_OPEN_FILES=1
export MELANIA_SMOKE_OPEN_START=1
export WLR_BACKENDS=x11
export LIBGL_ALWAYS_SOFTWARE=1
export GALLIUM_DRIVER=llvmpipe
export GSK_RENDERER=cairo
pid=''
socket_path=''
cleanup() {
  if [[ -n "$socket_path" && -S "$socket_path" ]]; then
    python3 - "$socket_path" <<'PY' || true
import socket,sys
with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as s:
    s.settimeout(.4)
    s.connect(sys.argv[1])
    s.sendall(b"QUIT\n")
    s.shutdown(socket.SHUT_WR)
    s.recv(256)
PY
  fi
  if [[ -n "$pid" ]]; then
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  fi
  mkdir -p "$ROOT/reports"
  cp "$temp/session.log" "$ROOT/reports/melania-labwc-session.log" 2>/dev/null || true
  rm -rf "$temp"
}
trap cleanup EXIT

xvfb-run -a -s '-screen 0 1280x800x24' dbus-run-session -- \
  bash "$ROOT/scripts/melania-labwc-session" --nested-test \
  >"$temp/session.log" 2>&1 &
pid=$!

for _ in {1..140}; do
  if ! kill -0 "$pid" 2>/dev/null; then
    cat "$temp/session.log" >&2
    echo "MELANIA_LABWC_SMOKE_FAIL=session_exited_early" >&2
    exit 1
  fi
  socket_path="$(find "$temp/runtime" -maxdepth 1 -type s \
    -name 'melania-labwc-*.sock' ! -name '*.start.sock' -print -quit)"
  [[ -z "$socket_path" ]] || break
  sleep .1
done
[[ -n "$socket_path" ]] || {
  cat "$temp/session.log" >&2
  echo "MELANIA_LABWC_SMOKE_FAIL=missing_private_panel_adapter" >&2
  exit 1
}
# The adapter binds before the supervisor starts both GTK layers.
# Waiting only for the socket is not evidence of an initialized shell.
shell_ready=0
for _ in {1..110}; do
  if grep -q 'MELANIA_LABWC_SHELL_STARTED=true' "$temp/session.log"; then
    shell_ready=1
    break
  fi
  kill -0 "$pid" 2>/dev/null || break
  sleep .1
done
if (( ! shell_ready )); then
  cat "$temp/session.log" >&2
  echo "MELANIA_LABWC_SMOKE_FAIL=no_shell_processes" >&2
  exit 1
fi
echo "MELANIA_LABWC_REAL_WAYLAND_SHELL_PROCESSES=true"

window_seen=0
for _ in {1..85}; do
  if python3 - "$socket_path" <<'PY'
import socket,sys
with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as s:
    s.settimeout(.7)
    s.connect(sys.argv[1])
    s.sendall(b"LIST\n")
    s.shutdown(socket.SHUT_WR)
    data=bytearray()
    while b"END\n" not in data:
        block=s.recv(4096)
        if not block: break
        data.extend(block)
    raw=data.decode("ascii")
    assert raw.startswith("OK\n"),raw
    assert raw.endswith("END\n"),raw
    assert "WINDOW\t" in raw,raw
    assert "org.melaniadesktop.Files" in raw or "melania-files" in raw.lower(),raw
PY
  then
    window_seen=1
    break
  fi
  kill -0 "$pid" 2>/dev/null || break
  sleep .15
done
if (( ! window_seen )); then
  cat "$temp/session.log" >&2
  echo "MELANIA_LABWC_SMOKE_FAIL=no_real_Wayland_Files_window_in_taskbar" >&2
  exit 1
fi
echo "MELANIA_LABWC_REAL_WAYLAND_TOPLEVEL_IN_TASKBAR=true"

hotkey_path="${socket_path%.sock}.start.sock"
start_ready=0
for _ in {1..35}; do
  [[ -S "$hotkey_path" ]] && { start_ready=1; break; }
  sleep .1
done
(( start_ready )) || { echo "MELANIA_LABWC_SMOKE_FAIL=no_Start_hotkey_listener" >&2; exit 1; }
# This CI run explicitly starts the panel with MELANIA_SMOKE_OPEN_START=1.
# The first real datagram therefore CLOSES an already-open Start. Verify a
# second datagram reopens it instead of incorrectly expecting open=true after
# just one toggle. The log must demonstrate both real transitions.
MELANIA_CONTROL_SOCKET="$socket_path" \
  python3 "$ROOT/scripts/melania-labwc-control.py" --toggle-start
for _ in {1..50}; do
  if grep -q 'MELANIA_START_IPC toggled open=false' "$temp/session.log"; then break; fi
  sleep .1
done
if ! grep -q 'MELANIA_START_IPC toggled open=false' "$temp/session.log"; then
  cat "$temp/session.log" >&2
  echo "MELANIA_LABWC_SMOKE_FAIL=Start_did_not_close" >&2
  exit 1
fi
MELANIA_CONTROL_SOCKET="$socket_path" \
  python3 "$ROOT/scripts/melania-labwc-control.py" --toggle-start
for _ in {1..50}; do
  if grep -q 'MELANIA_START_IPC toggled open=true' "$temp/session.log"; then break; fi
  sleep .1
done
if ! grep -q 'MELANIA_START_IPC toggled open=true' "$temp/session.log"; then
  cat "$temp/session.log" >&2
  echo "MELANIA_LABWC_SMOKE_FAIL=Start_did_not_reopen" >&2
  exit 1
fi
echo "MELANIA_LABWC_REAL_START_TOGGLE=true"

# Real pixels captured from Xvfb hosting the actual nested labwc/Wayland
# session, after the GTK Start overlay and Melania Files have been mapped.
mkdir -p "$ROOT/reports"
screenshot="$ROOT/reports/melania-labwc-live-session.png"
rm -f "$screenshot"
sleep .35
scrot "$screenshot"
python3 - "$screenshot" <<'PYSHOT'
from PIL import Image
from pathlib import Path
import sys
path=Path(sys.argv[1])
assert path.is_file() and path.stat().st_size>3000
image=Image.open(path).convert("RGB")
assert image.width>=1024 and image.height>=720, image.size
sample=image.resize((256,160))
assert len(sample.getcolors(256*160) or [])>80, "display is blank or nearly monochrome"
print("MELANIA_LABWC_REAL_SESSION_SCREENSHOT=" + str(path))
print("MELANIA_LABWC_REAL_PIXELS_VERIFIED=true")
PYSHOT

python3 - "$socket_path" <<'PY'
import socket,sys
with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as s:
    s.settimeout(.7)
    s.connect(sys.argv[1])
    s.sendall(b"QUIT\n")
    s.shutdown(socket.SHUT_WR)
    assert s.recv(100)==b"OK\nEND\n"
PY
for _ in {1..80}; do
  if ! kill -0 "$pid" 2>/dev/null; then break; fi
  sleep .1
done
if kill -0 "$pid" 2>/dev/null; then
  echo "MELANIA_LABWC_SMOKE_FAIL=logout_did_not_terminate_session" >&2
  exit 1
fi
wait "$pid" || {
  cat "$temp/session.log" >&2
  echo "MELANIA_LABWC_SMOKE_FAIL=labwc_exit_failed" >&2
  exit 1
}
pid=''
socket_path=''
echo "MELANIA_LABWC_REAL_LOGOUT_PASS=true"
echo "MELANIA_LABWC_REAL_NESTED_SESSION_PASS=true"
