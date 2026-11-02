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
fb_probe="$root/target/debug/melania-qemu-fb2-probe"
[[ -f "$fb_probe" && -x "$fb_probe" ]] ||
  deny 'build QEMU-only framebuffer GEM audit probe first'
visible_probe="$root/target/debug/melania-qemu-visible-probe"
[[ -f "$visible_probe" && -x "$visible_probe" ]] ||
  deny 'compile the QEMU-only visible-frame probe first'
[[ -f "$root/scripts/capture-qemu-visible-scanout.py" ]] ||
  deny 'QEMU QMP screenshot verifier unavailable'
shell_probe="$root/target/debug/qemu-shell-client"
[[ -x "$shell_probe" ]] || deny 'build Rust xdg/layer-shell QEMU client first'
pixel_client="$root/target/debug/qemu-client-window"
[[ -x "$pixel_client" ]] || deny 'build Rust Wayland wl_shm window client first'
stream_kms="$root/target/debug/qemu-rust-kms-stream"
[[ -x "$stream_kms" ]] || deny 'build live Rust KMS consumer first'
[[ -f "$root/scripts/capture-qemu-live-kms-stream.py" ]] ||
  deny 'QMP live stream screenshot verifier missing'
rust_kms="$root/target/debug/qemu-rust-kms-frames"
[[ -x "$rust_kms" ]] || deny 'build Rust DRM KMS page-flip executable first'
[[ -f "$root/scripts/capture-qemu-rust-kms-frames.py" ]] ||
  deny 'real two-frame QMP capture verifier missing'
pixel_scanout="$root/target/debug/melania-qemu-client-window-scanout"
[[ -x "$pixel_scanout" ]] || deny 'build QEMU-only real-client scanout program first'
[[ -f "$root/scripts/capture-qemu-client-window.py" ]] ||
  deny 'real client QMP screenshot validator missing'
client_probe="$root/target/debug/melania-qemu-wayland-client"
[[ -f "$client_probe" && -x "$client_probe" ]] ||
  deny 'build real libwayland guest client first'
[[ -f "$root/scripts/inject-qemu-native-input.py" ]] ||
  deny 'QMP virtual USB event injector missing'
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
  -graft-points "melania-native=$native" "melania-vt-guest.py=$root/scripts/melania-vt-guest.py" "melania-fb2-probe=$fb_probe" "melania-visible-probe=$visible_probe" "melania-wayland-client=$client_probe" "melania-shell-client=$shell_probe" "melania-pixel-client=$pixel_client" "melania-pixel-scanout=$pixel_scanout" "melania-rust-kms=$rust_kms" "melania-rust-stream=$stream_kms"
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
# QMP uses only a private Unix socket attached to the disposable QEMU VM.
# Capture actual virtual display pixels during the brief live KMS frame.
python3 "$root/scripts/capture-qemu-visible-scanout.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --output "$report/visible-frame.ppm" --timeout 950 \
  > "$report/qmp-scanout.log" 2>&1 &
capture_pid=$!
python3 "$root/scripts/inject-qemu-native-input.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" --timeout 950 \
  > "$report/qmp-input.log" 2>&1 &
input_pid=$!
python3 "$root/scripts/capture-qemu-client-window.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --output "$report/client-window.ppm" --timeout 950 \
  > "$report/qmp-client-window.log" 2>&1 &
client_capture_pid=$!
python3 "$root/scripts/capture-qemu-rust-kms-frames.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --first "$report/rust-client-frame1.ppm" \
  --second "$report/rust-client-frame2.ppm" --timeout 950 \
  > "$report/qmp-rust-frames.log" 2>&1 &
rust_capture_pid=$!
python3 "$root/scripts/capture-qemu-live-kms-stream.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --first "$report/live-client-frame1.ppm" \
  --second "$report/live-client-frame2.ppm" \
  --third "$report/live-client-frame3.ppm" --timeout 950 \
  > "$report/qmp-live-stream.log" 2>&1 &
live_capture_pid=$!
set +e
timeout --signal=TERM --kill-after=10s 1020s \
  qemu-system-x86_64 \
  -nodefaults -machine q35,accel=tcg -cpu max -smp 2 -m 3072 \
  -display none -vga virtio -serial "file:$serial" -monitor none -no-reboot \
  -qmp "unix:$work/melania-qmp.sock,server=on,wait=off" \
  -device qemu-xhci,id=melania-usb \
  -device usb-kbd,bus=melania-usb.0 \
  -device usb-tablet,bus=melania-usb.0 \
  -device virtio-scsi-pci,id=scsi \
  -drive if=none,file=overlay.qcow2,id=rootdisk,format=qcow2 \
  -device scsi-hd,drive=rootdisk \
  -drive if=none,file=seed.iso,id=seed,format=raw,media=cdrom \
  -device scsi-cd,drive=seed \
  -drive if=none,file=melania-payload.iso,id=payload,format=raw,media=cdrom \
  -device scsi-cd,drive=payload \
  -netdev user,id=net0 -device virtio-net-pci,netdev=net0
qemu_code=$?
wait "$capture_pid"
capture_code=$?
wait "$input_pid"
input_code=$?
wait "$client_capture_pid"
client_capture_code=$?
wait "$rust_capture_pid"
rust_capture_code=$?
wait "$live_capture_pid"
live_capture_code=$?
set -e
printf 'QEMU_EXIT=%s\n' "$qemu_code" > "$report/result.txt"
if [[ "$qemu_code" -ne 0 || "$capture_code" -ne 0 || "$input_code" -ne 0 || "$client_capture_code" -ne 0 || "$rust_capture_code" -ne 0 || "$live_capture_code" -ne 0 ]] ||
   ! grep -q '^MELANIA_QEMU_LIVE_KMS_THREE_CAPTURE_PASS=true' "$report/qmp-live-stream.log" ||
   ! grep -q '^MELANIA_QEMU_LIVE_KMS_FRAME1_PIXELS=true' "$report/qmp-live-stream.log" ||
   ! grep -q '^MELANIA_QEMU_LIVE_KMS_FRAME2_PIXELS=true' "$report/qmp-live-stream.log" ||
   ! grep -q '^MELANIA_QEMU_LIVE_KMS_FRAME3_PIXELS=true' "$report/qmp-live-stream.log" ||
   ! test -s "$report/live-client-frame1.ppm" ||
   ! test -s "$report/live-client-frame2.ppm" ||
   ! test -s "$report/live-client-frame3.ppm" ||
   ! grep -q '^MELANIA_NATIVE_FRAME_STREAM_SOCKET_READY=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_FRAME_STREAM_CONNECTED=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_CLIENT_FRAME_COUNT=3' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_KMS_STREAM_THREE_FRAMES=true' "$serial" ||
   ! grep -q '^MELANIA_RUST_STREAM_FRAME1_ACTIVE=true' "$serial" ||
   ! grep -q '^MELANIA_RUST_STREAM_FRAME2_ACTIVE=true' "$serial" ||
   ! grep -q '^MELANIA_RUST_STREAM_FRAME3_ACTIVE=true' "$serial" ||
   ! grep -q '^MELANIA_RUST_STREAM_THREE_CLIENT_FRAMES_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_LIVE_WAYLAND_KMS_STREAM_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_RUST_KMS_TWO_CAPTURE_PASS=true' "$report/qmp-rust-frames.log" ||
   ! grep -q '^MELANIA_QEMU_RUST_KMS_FRAME1_PIXELS=true' "$report/qmp-rust-frames.log" ||
   ! grep -q '^MELANIA_QEMU_RUST_KMS_FRAME2_PIXELS=true' "$report/qmp-rust-frames.log" ||
   ! test -s "$report/rust-client-frame1.ppm" ||
   ! test -s "$report/rust-client-frame2.ppm" ||
   ! grep -q '^MELANIA_NATIVE_CLIENT_FRAME_COUNT=2' "$serial" ||
   ! grep -q '^MELANIA_RUST_KMS_FRAME1_ACTIVE=true' "$serial" ||
   ! grep -q '^MELANIA_RUST_KMS_FRAME2_ACTIVE=true' "$serial" ||
   ! grep -q '^MELANIA_RUST_KMS_RESTORE_VERIFIED=true' "$serial" ||
   ! grep -q '^MELANIA_RUST_KMS_TWO_CLIENT_FRAMES_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_RUST_DIRECT_KMS_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_CLIENT_PIXELS_VERIFIED=true' "$report/qmp-client-window.log" ||
   ! test -s "$report/client-window.ppm" ||
   ! grep -q '^MELANIA_NATIVE_WL_OUTPUT_ADVERTISED=1280x800' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_CLIENT_WL_OUTPUT_MODE=1280x800' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_WL_OUTPUT_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_CLIENT_WL_SHM_RELEASED=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_CLIENT_XDG_WINDOW_DRAWN=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_CLIENT_SHM_IMPORTED=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_CLIENT_FRAME_COMPOSED=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_CLIENT_FRAME_SNAPSHOT_READY=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_RUST_CLIENT_FRAME_COMPOSED=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_CLIENT_WINDOW_SCANOUT_ACTIVE=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_CLIENT_WINDOW_RESTORE_VERIFIED=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_CLIENT_WINDOW_RESULT=PASS' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_CLIENT_WINDOW_SCANOUT_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_LIVE_KMS_RENDERING=false' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_USB_INPUT_INJECTED=true' "$report/qmp-input.log" ||
   ! grep -q '^MELANIA_NATIVE_WAYLAND_CLIENT_READY=true' "$serial" ||
   ! grep -Eq '^MELANIA_NATIVE_WAYLAND_CLIENT_KEY_PRESSES=[1-9]' "$serial" ||
   ! grep -Eq '^MELANIA_NATIVE_WAYLAND_CLIENT_POINTER_PRESSES=[1-9]' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_WAYLAND_CLIENT_INPUT_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_WAYLAND_INPUT_FORWARDED=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_WAYLAND_INPUT_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_XDG_CONFIGURE_ACK=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_LAYER_CONFIGURE_ACK=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_SHELL_PROTOCOL_CLIENT_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_SHELL_PROTOCOLS_READY=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_SHELL_RENDERED=false' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_SHELL_PROBE_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_WAYLAND_RENDER_READY=false' "$serial" ||
   ! grep -q '^MELANIA_QEMU_VISIBLE_PIXELS_VERIFIED=true' "$report/qmp-scanout.log" ||
   ! test -s "$report/visible-frame.ppm" ||
   ! grep -q '^MELANIA_QEMU_VISIBLE_SCANOUT_ACTIVE=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_VISIBLE_RESTORE_VERIFIED=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_VISIBLE_RESULT=PASS' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_VISIBLE_EXPERIMENT_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_DRM_PRESENT=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_SEATD_READY=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_CAPABILITIES=true' "$serial" ||
   ! grep -Eq '^MELANIA_NATIVE_INPUT_ENUMERATED keyboards=[1-9][0-9]* pointers=[1-9][0-9]* touch=[0-9]+ forwarded=false' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_INPUT_WAYLAND_READY=false' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_LIBINPUT_SESSION_PASS=true' "$serial" ||
   ! grep -q 'Melania QEMU atomic TEST_ONLY passed:' "$serial" ||
   ! grep -q 'baseline_unchanged=true' "$serial" ||
   ! grep -q 'resources_unchanged=true' "$serial" ||
   ! grep -q 'rollback_blockers=4' "$serial" ||
   ! grep -q 'restore_ready=false visible=false' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_KMS_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_FB2_ROUNDTRIP=PASS' "$serial" ||
   ! grep -Eq '^MELANIA_FB2_GEM_HANDLES_ACQUIRED=[1-9][0-9]*[[:space:]]*$' "$serial" ||
   ! grep -q '^MELANIA_FB2_ROLLBACK_READY=false' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_FB2_GEM_CLOSE_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_VT_SWITCH=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_VT_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_POST_KMS_VT_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_GUEST_EXIT=0' "$serial"; then
  printf '%s\n' 'MELANIA_QEMU_NATIVE_RESULT=FAIL' | tee -a "$report/result.txt"
  tail -n 100 "$serial" || true
  tail -n 30 "$report/qmp-scanout.log" || true
  tail -n 30 "$report/qmp-input.log" || true
  tail -n 30 "$report/qmp-client-window.log" || true
  tail -n 30 "$report/qmp-rust-frames.log" || true
  tail -n 30 "$report/qmp-live-stream.log" || true
  exit 1
fi
printf '%s\n' 'MELANIA_QEMU_NATIVE_RESULT=PASS' | tee -a "$report/result.txt"
printf '%s\n' 'MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_ONLY_VISIBLE_FRAME_AND_CRTC_RESTORE=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_WAYLAND_REAL_INPUT_DELIVERY=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_XDG_LAYER_SHELL_HANDSHAKE=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_CLIENT_PIXELS_RUST_COMPOSED_C_SCANOUT=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_NATIVE_WL_OUTPUT_MODE_VERIFIED=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_RUST_DIRECT_KMS_TWO_CLIENT_FRAMES=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_LIVE_WAYLAND_STREAM_THREE_FRAMES=true' >> "$report/result.txt"
