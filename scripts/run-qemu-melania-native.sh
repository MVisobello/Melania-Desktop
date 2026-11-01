#!/usr/bin/env bash
# Isolated QEMU TCG VM; actually execute host-built melania-native IN the guest.
# Does not open host DRM, use /dev/kvm, or enable GPU passthrough.
set -euo pipefail
deny() { printf 'MELANIA_QEMU_NATIVE_HOST_BLOCKED: %s\n' "$*" >&2; exit 78; }
[[ "${MELANIA_QEMU_NATIVE_VM:-}" == "YES_DISPOSABLE_QEMU" ]] ||
  deny 'explicit MELANIA_QEMU_NATIVE_VM=YES_DISPOSABLE_QEMU required'
for bin in qemu-system-x86_64 qemu-img cloud-localds genisoimage curl timeout sha256sum; do
  command -v "$bin" >/dev/null || deny "$bin missing"
done
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
native="$root/target/debug/melania-native"
[[ -f "$native" && -x "$native" ]] || deny 'build actual cargo -p melania-native first'
report="${MELANIA_QEMU_NATIVE_REPORT_DIR:-$root/reports/qemu-native}"
mkdir -p "$report"
report="$(realpath "$report")"
work="$(mktemp -d -t melania-native-guest-XXXXXXXX)"
trap 'rm -rf "$work"' EXIT

cd "$work"
image_base='https://cloud-images.ubuntu.com/noble/current'
image='noble-server-cloudimg-amd64.img'
curl --fail --location --retry 3 --connect-timeout 25 --max-time 420 -o "$image" "$image_base/$image"
curl --fail --location --retry 3 --connect-timeout 25 --max-time 60 -o SHA256SUMS "$image_base/SHA256SUMS"
grep -E "[[:space:]\\*]${image//./\\.}$" SHA256SUMS > verify.sha256 ||
  deny 'official image checksum entry missing'
sha256sum -c verify.sha256

qemu-img create -q -f qcow2 -F qcow2 -b "$work/$image" overlay.qcow2
# The binary is built on Ubuntu 24.04; transfer it through an ISO that is
# read-only inside QEMU. Not a host device mount; no binaries downloaded
# from arbitrary sources at guest runtime.
genisoimage -quiet -R -J -V MELANIA_BIN -o melania-payload.iso \
  -graft-points "melania-native=$native" "melania-vt-guest.py=$root/scripts/melania-vt-guest.py"
printf '%s\n' 'instance-id: melania-native-stage17' 'local-hostname: melania-native-qemu' > meta-data
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
write_files:
  - path: /usr/local/sbin/melania-qemu-native-guest.sh
    permissions: '0700'
    encoding: b64
    content: $encoded
runcmd:
  - [ bash, -c, 'modprobe virtio_gpu 2>/dev/null || true; bash /usr/local/sbin/melania-qemu-native-guest.sh > /dev/ttyS0 2>&1; code=\$?; echo "MELANIA_QEMU_NATIVE_GUEST_EXIT=\$code" > /dev/ttyS0; sync; poweroff -f' ]
EOF
cloud-localds seed.iso user-data meta-data

serial="$report/serial.log"
: > "$serial"
set +e
timeout --signal=TERM --kill-after=10s 1020s \
  qemu-system-x86_64 \
  -nodefaults -machine q35,accel=tcg -cpu max -smp 2 -m 3072 \
  -display none -vga virtio -serial "file:$serial" -monitor none -no-reboot \
  -device virtio-scsi-pci,id=scsi \
  -drive if=none,file=overlay.qcow2,id=rootdisk,format=qcow2 \
  -device scsi-hd,drive=rootdisk \
  -drive if=none,file=seed.iso,id=seed,format=raw,media=cdrom \
  -device scsi-cd,drive=seed \
  -drive if=none,file=melania-payload.iso,id=payload,format=raw,media=cdrom \
  -device scsi-cd,drive=payload \
  -netdev user,id=net0 -device virtio-net-pci,netdev=net0
qemu_code=$?
set -e
printf 'QEMU_EXIT=%s\n' "$qemu_code" > "$report/result.txt"
if [[ "$qemu_code" -ne 0 ]] ||
   ! grep -q '^MELANIA_QEMU_NATIVE_DRM_PRESENT=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_SEATD_READY=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_CAPABILITIES=true' "$serial" ||
   ! grep -q 'Melania QEMU atomic TEST_ONLY passed:' "$serial" ||
   ! grep -q 'baseline_unchanged=true' "$serial" ||
   ! grep -q 'resources_unchanged=true' "$serial" ||
   ! grep -q 'rollback_blockers=4' "$serial" ||
   ! grep -q 'restore_ready=false visible=false' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_KMS_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_VT_SWITCH=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_VT_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_POST_KMS_VT_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_GUEST_EXIT=0' "$serial"; then
  printf '%s\n' 'MELANIA_QEMU_NATIVE_RESULT=FAIL' | tee -a "$report/result.txt"
  tail -n 100 "$serial" || true
  exit 1
fi
printf '%s\n' 'MELANIA_QEMU_NATIVE_RESULT=PASS' | tee -a "$report/result.txt"
printf '%s\n' 'MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false' >> "$report/result.txt"
