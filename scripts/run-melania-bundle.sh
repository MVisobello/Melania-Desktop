#!/usr/bin/env bash
# Run the precompiled Melania nested preview inside an existing graphical session.
# Intentionally does not install a display-manager entry or acquire DRM/VT devices.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
BIN="$ROOT/bin"

preflight() {
  [[ "$(uname -s)" == Linux ]] || { echo 'BLOCKED: Linux is required' >&2; return 78; }
  [[ -n "${WAYLAND_DISPLAY:-}" || -n "${DISPLAY:-}" ]] || {
    echo 'BLOCKED: an existing graphical host (Wayland/X11) is required' >&2
    return 78
  }
  [[ -n "${XDG_RUNTIME_DIR:-}" && -d "$XDG_RUNTIME_DIR" ]] || {
    echo 'BLOCKED: missing host XDG_RUNTIME_DIR' >&2
    return 78
  }
  command -v dbus-run-session >/dev/null || {
    echo 'BLOCKED: dbus-run-session is required' >&2
    return 78
  }
  local binary
  for binary in melania-compositor melania-desktop-surface melania-panel \
                melania-files melania-settings melania-winctl \
                melania-notifyd melania-notifyctl; do
    [[ -f "$BIN/$binary" && -x "$BIN/$binary" ]] || {
      echo "BLOCKED: missing executable in bundle: $binary" >&2
      return 78
    }
  done
  [[ -f "$ROOT/share/wallpaper.png" ]] || {
    echo 'BLOCKED: missing bundled wallpaper' >&2
    return 78
  }
  # Mint 22.x builds may contain one privately compiled GTK4 layer-shell
  # bridge. Never overwrite or install GTK/glibc/Mesa into the host OS.
  if [[ -d "$ROOT/lib" ]]; then
    [[ -f "$ROOT/lib/libgtk4-layer-shell.so.0" &&
       ! -L "$ROOT/lib/libgtk4-layer-shell.so.0" ]] || {
      echo 'BLOCKED: bundled GTK4 layer-shell runtime missing or unsafe' >&2
      return 78
    }
    export LD_LIBRARY_PATH="$ROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
  fi
  if command -v ldd >/dev/null 2>&1; then
    local diagnostic
    for binary in melania-compositor melania-desktop-surface melania-panel \
                  melania-files melania-settings melania-winctl \
                  melania-notifyd melania-notifyctl; do
      diagnostic="$(ldd "$BIN/$binary" 2>&1)" || {
        echo "BLOCKED: incompatible binary or dynamic loader: $binary" >&2
        echo "$diagnostic" >&2
        return 78
      }
      if grep -Eq 'not found|version .* not found' <<<"$diagnostic"; then
        echo "BLOCKED: missing/incompatible Linux runtime for $binary" >&2
        grep -E 'not found|version .* not found' <<<"$diagnostic" >&2
        echo 'On Linux Mint install the missing system runtime packages; never replace glibc.' >&2
        return 78
      fi
    done
  fi
  echo 'MELANIA_NESTED_BUNDLE_PREFLIGHT_OK=true'
  echo 'MELANIA_NATIVE_LOGIN_SESSION_READY=false'
}

case "${1:-}" in
  --check) [[ $# -eq 1 ]] || exit 2; preflight; exit $? ;;
  "") [[ $# -eq 0 ]] || exit 2 ;;
  *) echo 'Usage: ./run.sh [--check]' >&2; exit 2 ;;
esac

preflight
export MELANIA_BUNDLE_ROOT="$ROOT"
export MELANIA_BUNDLE_DATA_DIR="$ROOT/share"
export PATH="$BIN:$PATH"
# A private D-Bus session is used for the preview; the existing Linux graphical
# session and login manager are never replaced or modified.
exec dbus-run-session -- bash -euo pipefail -c '
  "$MELANIA_BUNDLE_ROOT/bin/melania-notifyd" &
  NOTIFY_PID=$!
  cleanup() {
    trap - EXIT
    kill "$NOTIFY_PID" 2>/dev/null || true
    wait "$NOTIFY_PID" 2>/dev/null || true
  }
  trap cleanup EXIT
  "$MELANIA_BUNDLE_ROOT/bin/melania-compositor" \
    --command "$MELANIA_BUNDLE_ROOT/bin/melania-desktop-surface" \
    --command "$MELANIA_BUNDLE_ROOT/bin/melania-panel" \
    --command "$MELANIA_BUNDLE_ROOT/bin/melania-files" \
    --command "$MELANIA_BUNDLE_ROOT/bin/melania-settings"
'
