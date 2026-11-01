#!/usr/bin/env bash
# Stage12 opt-in QEMU-only DRM resource preflight. Not a display restore test.
# This helper NEVER starts QEMU or opens the host DRM device itself.
set -euo pipefail

deny() { echo "NATIVE_STAGE12_BLOCKED: $*" >&2; exit 78; }
[[ "${MELANIA_QEMU_RESOURCE_AUDIT:-}" == "I_AM_IN_AN_ISOLATED_QEMU_VM" ]] ||
  deny "explicit isolated-QEMU acknowledgement required"
[[ -z "${DISPLAY:-}" && -z "${WAYLAND_DISPLAY:-}" ]] ||
  deny "do not run inside an existing desktop"
[[ "$(tty 2>/dev/null || true)" =~ ^/dev/tty[0-9]+$ ]] ||
  deny "a real dedicated /dev/ttyN console is required"
[[ "${MELANIA_EXPERIMENTAL_SEAT:-}" == "1" ]] ||
  deny "MELANIA_EXPERIMENTAL_SEAT=1 required"
[[ "${MELANIA_KMS_TEST_ONLY:-}" == "YES_QEMU_VM" ]] ||
  deny "MELANIA_KMS_TEST_ONLY=YES_QEMU_VM required"

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binary="$root/target/debug/melania-native"
[[ -x "$binary" ]] || deny "compile melania-native inside the isolated QEMU VM first"
command -v timeout >/dev/null || deny "GNU timeout required for bounded experiment"

# Firmware and DRM gating is repeated inside melania-native; a spoofed shell
# variable cannot bypass the executable's DMI/QEMU/VT safeguards.
echo "Stage12 QEMU: read-only KMS snapshots, mode/color/HDR bytes and FB observations"
echo "No atomic commit, modeset, page flip, or framebuffer restoration is enabled."
timeout --signal=TERM --kill-after=3s 45s "$binary" --experimental-kms-test
