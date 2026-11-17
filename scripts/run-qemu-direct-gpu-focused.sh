#!/usr/bin/env bash
# Focused, actual native GPU acceptance on a disposable isolated QEMU VM.
# This is not a replacement for the full native KMS/Wayland regression suite.
set -euo pipefail
[[ "${MELANIA_QEMU_NATIVE_VM:-}" == YES_DISPOSABLE_QEMU ]] ||
  { echo 'Explicit disposable QEMU opt-in required' >&2; exit 78; }
case "${MELANIA_QEMU_FOCUSED_MODE:-direct}" in
  direct)
    guest_arg=--direct-gpu-only
    proof=MELANIA_QEMU_DIRECT_CLIENT_GPU_TO_GBM_KMS_12_FRAMES_PASS=true
    focused=MELANIA_QEMU_FOCUSED_DIRECT_GPU_PASS=true ;;
  retained)
    guest_arg=--retained-gpu-only
    proof=MELANIA_QEMU_RETAINED_FOUR_WINDOW_GPU_KMS_12_FRAMES_PASS=true
    focused=MELANIA_QEMU_FOCUSED_RETAINED_GPU_PASS=true ;;
  *) echo 'Unknown focused GPU mode' >&2; exit 78 ;;
esac
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
native="$root/target/debug/melania-native"
client="$root/target/debug/qemu-continuous-client"
[[ -x "$native" && -x "$client" ]] || { echo 'Build real native server/client first' >&2; exit 78; }
report="${MELANIA_QEMU_NATIVE_REPORT_DIR:-$root/reports/qemu-direct-gpu}"
mkdir -p "$report"
report="$(realpath "$report")"
work="$(mktemp -d -t melania-direct-gpu-XXXXXXXX)"
trap 'rm -rf "$work"' EXIT
cd "$work"
base=https://cloud-images.ubuntu.com/noble/current
image=noble-server-cloudimg-amd64.img
curl --fail --location --retry 3 --connect-timeout 25 --max-time 420 -o "$image" "$base/$image"
curl --fail --location --retry 3 --connect-timeout 25 --max-time 60 -o SHA256SUMS "$base/SHA256SUMS"
grep -E "[[:space:]\\*]${image//./\\.}$" SHA256SUMS > verify.sha256
sha256sum -c verify.sha256
qemu-img create -q -f qcow2 -F qcow2 -b "$work/$image" overlay.qcow2
genisoimage -quiet -R -J -V MELANIA_BIN -o payload.iso -graft-points \
  "melania-native=$native" "melania-continuous-client=$client"
printf '%s\n' 'instance-id: melania-direct-gpu-focused' 'local-hostname: melania-native-qemu' > meta-data
encoded="$(base64 -w0 "$root/scripts/melania-qemu-native-guest.sh")"
cat > user-data <<EOF
#cloud-config
ssh_pwauth: false
disable_root: false
packages:
  - seatd
  - libseat1
  - libinput10
  - libudev1
  - libdrm2
  - libgbm1
  - libegl1
  - libgles2
  - libxkbcommon0
  - libwayland-client0
  - libwayland-server0
  - libdisplay-info1
  - gdb
write_files:
  - path: /usr/local/sbin/melania-qemu-native-guest.sh
    permissions: '0700'
    encoding: b64
    content: $encoded
runcmd:
  - [ bash, -c, 'modprobe virtio_gpu 2>/dev/null || true; MELANIA_QEMU_DIRECT_GDB=YES_QEMU_VM bash /usr/local/sbin/melania-qemu-native-guest.sh $guest_arg > /dev/ttyS0 2>&1; code=\$?; echo "MELANIA_QEMU_NATIVE_GUEST_EXIT=\$code" > /dev/ttyS0; sync; poweroff -f' ]
EOF
cloud-localds seed.iso user-data meta-data
serial="$report/serial.log"
: > "$serial"
drag_pid=''
if [[ "$guest_arg" == --retained-gpu-only ]]; then
  python3 "$root/scripts/inject-qemu-retained-gpu-drag.py" --serial "$serial" --qmp "$work/melania-qmp.sock" --timeout 650 > "$report/qmp-retained-drag.log" 2>&1 &
  drag_pid=$!
fi
set +e
timeout --signal=TERM --kill-after=10s 650s \
  qemu-system-x86_64 \
  -nodefaults -machine q35,accel=tcg -cpu max -smp 2 -m 3072 \
  -display none -vga virtio -serial "file:$serial" -monitor none -no-reboot \
  -qmp "unix:$work/melania-qmp.sock,server=on,wait=off" \
  -device qemu-xhci,id=melania-usb \
  -device usb-kbd,bus=melania-usb.0 -device usb-tablet,bus=melania-usb.0 \
  -device virtio-scsi-pci,id=scsi \
  -drive if=none,file=overlay.qcow2,id=rootdisk,format=qcow2 -device scsi-hd,drive=rootdisk \
  -drive if=none,file=seed.iso,id=seed,format=raw,media=cdrom -device scsi-cd,drive=seed \
  -drive if=none,file=payload.iso,id=payload,format=raw,media=cdrom -device scsi-cd,drive=payload \
  -netdev user,id=net0 -device virtio-net-pci,netdev=net0
status=$?
drag_status=0
if [[ -n "$drag_pid" ]]; then wait "$drag_pid"; drag_status=$?; fi
set -e
if [[ "$drag_status" != 0 ]]; then cat "$report/qmp-retained-drag.log"; exit 1; fi
cat "$serial"
# Preserve raw serial evidence; ttyS0 emits CRLF.
normalized="$report/serial.normalized.log"
sed 's/\r$//' "$serial" > "$normalized"
if [[ "$status" != 0 ]]; then
  echo "Focused real GPU diagnostic: QEMU exit $status" >&2
  exit 1
fi
for marker in 'MELANIA_QEMU_NATIVE_GUEST_EXIT=0' "$proof" "$focused"; do
  if ! grep -qxF "$marker" "$normalized"; then
    echo "Focused real GPU diagnostic: missing exact marker $marker" >&2
    exit 1
  fi
done
if [[ "$guest_arg" == --retained-gpu-only ]]; then
  grep -qxF 'MELANIA_QEMU_RETAINED_GPU_REAL_USB_DRAG_KMS_PASS=true' "$normalized"
  grep -qxF 'MELANIA_QEMU_RETAINED_GPU_LIVE_DRAG_KMS_PASS=true' "$normalized"
  grep -qxF 'MELANIA_QEMU_RETAINED_GPU_VISIBILITY_KMS_PASS=true' "$normalized"
  grep -qxF 'MELANIA_QEMU_RETAINED_GPU_STACK_KMS_PASS=true' "$normalized"
  grep -qxF 'MELANIA_QEMU_RETAINED_GPU_WAYLAND_FOCUS_PASS=true' "$normalized"
  grep -qxF 'MELANIA_QEMU_RETAINED_GPU_USB_RAISE_SENT=true' "$report/qmp-retained-drag.log"
  grep -qxF 'MELANIA_QEMU_RETAINED_GPU_USB_VISIBILITY_SENT=true' "$report/qmp-retained-drag.log"
fi
echo 'MELANIA_QEMU_FOCUSED_DIRECT_GPU_RESULT=PASS'
