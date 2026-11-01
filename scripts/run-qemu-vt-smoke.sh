#!/usr/bin/env bash
# Disposable Ubuntu QEMU VM test: real VT activation/restoration, no KMS commit.
set -euo pipefail
fail() { printf 'MELANIA_QEMU_HOST_BLOCKED: %s\n' "$*" >&2; exit 78; }
[[ "${MELANIA_QEMU_TTY_SMOKE:-}" == "YES_DISPOSABLE_QEMU" ]] ||
  fail 'explicit MELANIA_QEMU_TTY_SMOKE=YES_DISPOSABLE_QEMU required'
for bin in qemu-system-x86_64 qemu-img cloud-localds curl timeout sha256sum base64; do
  command -v "$bin" >/dev/null || fail "$bin is required"
done

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
report_dir="${MELANIA_QEMU_REPORT_DIR:-$root/reports/qemu-tty}"
mkdir -p "$report_dir"
report_dir="$(realpath "$report_dir")"
work="$(mktemp -d -t melania-qemu-tty-XXXXXXXX)"
trap 'rm -rf "$work"' EXIT

image_base='https://cloud-images.ubuntu.com/noble/current'
image='noble-server-cloudimg-amd64.img'
cd "$work"
# Download official Ubuntu image plus SHA256 verification (no arbitrary VM).
curl --fail --location --retry 3 --connect-timeout 25 --max-time 420 \
  -o "$image" "$image_base/$image"
curl --fail --location --retry 3 --connect-timeout 25 --max-time 60 \
  -o SHA256SUMS "$image_base/SHA256SUMS"
grep -E "[[:space:]\\*]${image//./\\.}$" SHA256SUMS > verify.sha256 ||
  fail 'cloud image missing from official checksum manifest'
sha256sum -c verify.sha256

qemu-img create -q -f qcow2 -F qcow2 -b "$work/$image" guest-overlay.qcow2
printf '%s\n' 'instance-id: melania-qemu-vt-smoke' \
  'local-hostname: melania-qemu-smoke' > meta-data
encoded="$(base64 -w 0 "$root/scripts/melania-vt-guest.py")"
cat > user-data <<EOF
#cloud-config
ssh_pwauth: false
disable_root: false
write_files:
  - path: /usr/local/sbin/melania-vt-guest.py
    permissions: '0700'
    encoding: b64
    content: $encoded
runcmd:
  - [ bash, -c, 'python3 /usr/local/sbin/melania-vt-guest.py > /dev/ttyS0 2>&1; rc=\$?; echo "MELANIA_QEMU_VT_EXIT=\$rc" > /dev/ttyS0; sync; poweroff -f' ]
EOF
cloud-localds seed.img user-data meta-data

# -display none keeps the virtual VGA/DRM device but leaves the host display
# untouched. TCG emulation deliberately does NOT require/pass through /dev/kvm.
serial="$report_dir/serial.log"
: > "$serial"
set +e
timeout --signal=TERM --kill-after=10s 840s \
  qemu-system-x86_64 \
  -nodefaults -machine q35,accel=tcg -cpu max -smp 2 -m 2048 \
  -display none -vga std -serial "file:$serial" -monitor none -no-reboot \
  -device virtio-scsi-pci,id=scsi \
  -drive if=none,file=guest-overlay.qcow2,id=drive0,format=qcow2 \
  -device scsi-hd,drive=drive0 \
  -drive if=none,file=seed.img,id=seed,format=raw,media=cdrom \
  -device scsi-cd,drive=seed \
  -netdev user,id=net0 -device virtio-net-pci,netdev=net0
qemu_code=$?
set -e
printf 'qemu_exit=%s\n' "$qemu_code" > "$report_dir/result.txt"
if [[ "$qemu_code" -ne 0 ]] ||
   ! grep -q '^MELANIA_QEMU_VGA_ENUMERATED=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_VT_SWITCH=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_VT_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_VT_EXIT=0' "$serial"; then
  printf '%s\n' 'MELANIA_QEMU_TTY_RESULT=FAIL' | tee -a "$report_dir/result.txt"
  tail -n 90 "$serial" || true
  exit 1
fi
printf '%s\n' 'MELANIA_QEMU_TTY_RESULT=PASS' | tee -a "$report_dir/result.txt"
# A VT roundtrip does NOT prove that DRM scanout, libseat or KMS rollback works.
printf '%s\n' 'MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false' >> "$report_dir/result.txt"
