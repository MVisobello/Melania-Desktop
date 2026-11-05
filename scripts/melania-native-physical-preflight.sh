#!/usr/bin/env bash
# Real, read-only dedicated-VT libseat/DRM/GBM smoke; NEVER a login test.
set -Eeuo pipefail
umask 077
deny() {
  printf 'MELANIA_PHYSICAL_PREFLIGHT_BLOCKED=%s\n' "$*" >&2
  printf 'NATIVE_SESSION_READY=false\n' >&2
  exit 78
}
[[ "${MELANIA_NATIVE_PHYSICAL_ACK:-}" == "READ_ONLY_HARDWARE_PREFLIGHT" ]] ||
  deny 'explicit read-only hardware acknowledgement required'
[[ "$EUID" -ne 0 ]] || deny 'run as the logged-in user, never sudo'
[[ -z "${DISPLAY:-}" && -z "${WAYLAND_DISPLAY:-}" ]] ||
  deny 'desktop session detected; use a free VT'
console="$(readlink /proc/self/fd/0)" || deny 'controlling TTY unavailable'
[[ "$console" =~ ^/dev/tty[1-9][0-9]*$ ]] ||
  deny 'dedicated /dev/ttyN required, not SSH or a terminal emulator'
[[ -n "${XDG_RUNTIME_DIR:-}" && -d "$XDG_RUNTIME_DIR" ]] ||
  deny 'XDG_RUNTIME_DIR missing'
[[ -d /dev/dri ]] || deny 'DRM device directory missing'
[[ -n "${HOME:-}" && -d "$HOME" ]] || deny 'HOME missing'
[[ "$(stat -c %u "$XDG_RUNTIME_DIR")" == "$(id -u)" ]] ||
  deny 'XDG_RUNTIME_DIR belongs to a different user'
if command -v systemd-detect-virt >/dev/null; then
  if systemd-detect-virt --vm --quiet; then
    deny 'VM detected: virtual DRM cannot certify physical hardware'
  fi
fi
command -v timeout >/dev/null || deny 'GNU timeout required'
root="$(cd "$(dirname "$0")/.." && pwd)"
binary="${MELANIA_NATIVE_BINARY:-$root/target/debug/melania-native}"
[[ -x "$binary" ]] || deny "build the native Rust binary first: $binary"
report="${MELANIA_NATIVE_PHYSICAL_REPORT_DIR:-$HOME/melania-native-hardware-$(date -u +%Y%m%dT%H%M%SZ)}"
mkdir -p "$report"
{
  printf 'UTC=%s\n' "$(date -u +%FT%TZ)"
  printf 'Kernel=%s\n' "$(uname -r)"
  printf 'Console=%s\n' "$console"
  printf 'NATIVE_SESSION_READY=false\n'
  printf 'NotTested=scanout, VT recovery, GPU hotplug, multi-CRTC rendering, client DMA-BUF, Xwayland, login\n'
} > "$report/manifest.txt"
failure=0
probe() {
  local name="$1" seconds="$2" marker="$3"
  shift 3
  printf 'MELANIA_PHYSICAL_TEST_START=%s\n' "$name"
  local code=0
  timeout --signal=TERM --kill-after=5s "$seconds" \
    env MELANIA_EXPERIMENTAL_SEAT=1 "$binary" "$@" \
    > "$report/$name.log" 2>&1 || code=$?
  if [[ "$code" -eq 0 ]] && grep -Eq "$marker" "$report/$name.log"; then
    printf 'MELANIA_PHYSICAL_TEST_PASS=%s\n' "$name"
    printf '%s=PASS\n' "$name" >> "$report/manifest.txt"
  else
    printf 'MELANIA_PHYSICAL_TEST_FAILED=%s exit=%s\n' "$name" "$code" >&2
    printf '%s=FAIL:%s\n' "$name" "$code" >> "$report/manifest.txt"
    failure=1
  fi
}
probe seat_enum 35 '^MELANIA_NATIVE_INPUT_ENUMERATED ' --experimental-seat
[[ "$failure" -eq 0 ]] || deny 'no validated live libseat input inventory; refusing GPU probes'
probe drm_routes 60 '^MELANIA_NATIVE_ACTIVE_DRM_ROUTES=[1-9][0-9]* \(READ_ONLY\)' --experimental-drm
[[ "$failure" -eq 0 ]] || deny 'no validated stable physical CRTC route; refusing GPU probes'
probe gbm_offscreen 120 'Melania native offscreen GLES frames: OK \(NOT displayed\)' --experimental-offscreen
probe gbm_output 120 '^MELANIA_NATIVE_GBM_OUTPUT_SIZED_SYNCHRONIZED=[0-9]+x[0-9]+x2$' --experimental-gbm-output
printf 'NATIVE_SESSION_READY=false\n' | tee -a "$report/manifest.txt"
printf 'MELANIA_PHYSICAL_PREFLIGHT_REPORT=%s\n' "$report"
[[ "$failure" -eq 0 ]] || exit 1
printf 'MELANIA_PHYSICAL_PREFLIGHT_COMPLETE=PASS_LIMITED\n'
# Success proves only the four tests above on this local GPU and seat.
# It cannot authorize the native compositor or disable the labwc fallback.
