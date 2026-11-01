#!/usr/bin/env bash
# QEMU guest ONLY. Actual Rust melania-native binary, never the host GPU.
# A failed DRM ioctl, seat acquisition, rendering or TEST_ONLY is a FAIL.
set -euo pipefail
report() { printf '%s\n' "$*"; }
fatal() { report "MELANIA_QEMU_NATIVE_FAIL=$*"; exit 1; }

[[ "$(id -u)" == "0" ]] || fatal 'root_required_inside_disposable_guest'
vendor="$(cat /sys/class/dmi/id/sys_vendor 2>/dev/null || true)"
model="$(cat /sys/class/dmi/id/product_name 2>/dev/null || true)"
[[ "$vendor $model" == *QEMU* || "$vendor $model" == *Bochs* || "$vendor $model" == *KVM* ]] ||
  fatal 'QEMU_DMI_required'
[[ -c /dev/tty1 && -c /dev/tty0 ]] || fatal 'real_Linux_virtual_console_missing'
[[ -c /dev/dri/card0 ]] || fatal 'virtual_DRM_card0_missing'
report 'MELANIA_QEMU_NATIVE_DRM_PRESENT=true'

mkdir -p /mnt/melania-native
mount -t iso9660 -o ro /dev/disk/by-label/MELANIA_BIN /mnt/melania-native ||
  fatal 'signed-off_immutable_payload_ISO_unavailable'
binary=/mnt/melania-native/melania-native
[[ -f "$binary" ]] || fatal 'real_native_ELF_binary_missing'
file "$binary" || true
mkdir -p /run/melania-native-qemu
chmod 0700 /run/melania-native-qemu

# libseat's seatd backend owns the DRM descriptor. It must run as a daemon
# inside this disposable VM; never open any device on the GitHub runner.
systemctl start seatd || fatal 'seatd_not_available'
[[ -S /run/seatd.sock ]] || fatal 'seatd_socket_unavailable'
report 'MELANIA_QEMU_NATIVE_SEATD_READY=true'

export LIBSEAT_BACKEND=seatd
export XDG_RUNTIME_DIR=/run/melania-native-qemu
export MELANIA_EXPERIMENTAL_SEAT=1
export MELANIA_KMS_TEST_ONLY=YES_QEMU_VM
unset DISPLAY WAYLAND_DISPLAY

# These tests deliberately enter with fd 0 from a real dedicated Linux VT,
# not the cloud-init serial console, a PTY, or SSH. The user-visible log is
# redirected to /dev/ttyS0 by cloud-init, preserving fd0=/dev/tty1.
timeout --signal=TERM --kill-after=3s 25s \
  "$binary" --capabilities < /dev/tty1 ||
  fatal 'native_capabilities_failed'
report 'MELANIA_QEMU_NATIVE_CAPABILITIES=true'

# Exercise the real libseat/udev/libinput event sources on the VM TTY.
# This stage enumerates device classes, but cannot inject any event into
# Wayland clients. It must exit cleanly before the KMS experiment starts.
input_log="$(mktemp -t melania-qemu-libinput-XXXXXX)"
if ! timeout --signal=TERM --kill-after=3s 18s \
  "$binary" --experimental-seat < /dev/tty1 | tee "$input_log"; then
  rm -f "$input_log"
  fatal 'native_libinput_session_smoke_failed'
fi
if ! grep -Eq '^MELANIA_NATIVE_INPUT_ENUMERATED keyboards=[1-9][0-9]* pointers=[1-9][0-9]* touch=[0-9]+ forwarded=false' "$input_log"; then
  rm -f "$input_log"
  fatal 'virtual_USB_keyboard_or_pointer_not_enumerated'
fi
rm -f "$input_log"
report 'MELANIA_QEMU_NATIVE_LIBINPUT_SESSION_PASS=true'

# Try the genuine Rust/Smithay KMS preflight, not a C substitute or mock.
# It is TEST_ONLY and never issues an atomic commit/page flip/scanout.
set +e
timeout --signal=TERM --kill-after=4s 70s \
  "$binary" --experimental-kms-test < /dev/tty1
code=$?
set -e
report "MELANIA_QEMU_NATIVE_KMS_EXIT=$code"
[[ "$code" -eq 0 ]] || fatal 'native_KMS_TEST_ONLY_failed'
report 'MELANIA_QEMU_NATIVE_KMS_PASS=true'

# Only AFTER the native process returns, confirm the console can still be
# changed and restored via Linux VT ioctl in the SAME disposable guest.
# This is a real VT test but NOT a rollback after visible scanout.
vt_probe=/mnt/melania-native/melania-vt-guest.py
[[ -f "$vt_probe" ]] || fatal 'vt_recovery_probe_missing_from_payload'
timeout --signal=TERM --kill-after=3s 25s python3 "$vt_probe" ||
  fatal 'post_KMS_linux_VT_recovery_failed'
report 'MELANIA_QEMU_NATIVE_POST_KMS_VT_PASS=true'
report 'MELANIA_NATIVE_SESSION_READY=false'
report 'MELANIA_NATIVE_VISIBLE_SCANOUT=false'
report 'MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false'
