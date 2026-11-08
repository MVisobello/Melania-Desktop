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
  # Never report an apparently healthy desktop if the host cannot perform
  # runtime linker checks: missing GTK4 layer-shell can leave only wallpaper.
  # `ldd` is a required preflight dependency, not an optional diagnostic.
  command -v ldd >/dev/null 2>&1 || {
    echo 'BLOCKED: ldd is required to check bundled Linux runtime libraries' >&2
    echo 'HINT: install the distribution-provided ldd (libc-bin on Ubuntu/Mint).' >&2
    return 78
  }
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
  echo 'MELANIA_NESTED_BUNDLE_PREFLIGHT_OK=true'
  echo 'MELANIA_NATIVE_LOGIN_SESSION_READY=false'
}

# A software graphics fallback is useful in VirtualBox guests whose virtual
# GL/EGL driver fails to initialize. Never change the host's GPU configuration.
# Automatic selection can be explicitly overridden by MELANIA_SOFTWARE_RENDER=0.
detect_virtualbox() {
  local product=''
  if [[ -r /sys/class/dmi/id/product_name ]]; then
    IFS= read -r product < /sys/class/dmi/id/product_name || true
  fi
  case "${product,,}" in
    *virtualbox*) return 0 ;;
  esac
  if command -v systemd-detect-virt >/dev/null 2>&1; then
    [[ "$(systemd-detect-virt --vm 2>/dev/null || true)" == oracle ]] && return 0
  fi
  return 1
}

configure_renderer() {
  local software="${MELANIA_SOFTWARE_RENDER:-auto}"
  case "$software" in
    auto) if detect_virtualbox; then software=1; else software=0; fi ;;
    1|0) ;;
    *) echo 'BLOCKED: MELANIA_SOFTWARE_RENDER must be 0, 1 or auto' >&2; return 2 ;;
  esac
  if [[ "$software" == 1 ]]; then
    # GTK4 cairo avoids guest Vulkan/Zink; llvmpipe provides GLES to Smithay.
    # These variables affect Melania children only, never system-wide libraries.
    export LIBGL_ALWAYS_SOFTWARE=1
    export GALLIUM_DRIVER=llvmpipe
    export MESA_LOADER_DRIVER_OVERRIDE=llvmpipe
    export GSK_RENDERER=cairo
    echo 'MELANIA_RENDERING_BACKEND=software (GTK cairo + Mesa llvmpipe)'
  else
    echo 'MELANIA_RENDERING_BACKEND=host'
  fi
}

case "${1:-}" in
  --check) [[ $# -eq 1 ]] || exit 2; preflight; configure_renderer; exit $? ;;
  --software) [[ $# -eq 1 ]] || exit 2; export MELANIA_SOFTWARE_RENDER=1 ;;
  "") [[ $# -eq 0 ]] || exit 2 ;;
  *) echo 'Usage: ./run.sh [--check|--software]' >&2; exit 2 ;;
esac

preflight
configure_renderer
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
