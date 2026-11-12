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
continuous_kms="$root/target/debug/qemu-rust-kms-continuous"
continuous_client="$root/target/debug/qemu-continuous-client"
two_continuous_client="$root/target/debug/qemu-continuous-two-windows"
ime_probe="$root/target/debug/qemu-ime-roundtrip"
[[ -x "$ime_probe" ]] || deny 'actual separate-process IME-v2 client ELF missing'
real_popup_client="$root/target/debug/qemu-xdg-popup-client"
[[ -x "$real_popup_client" ]] || deny 'build real xdg_popup Wayland QEMU client'
[[ -f "$root/scripts/capture-qemu-real-xdg-popup.py" ]] ||
  deny 'real QMP popup/close screenshot verifier unavailable'
auth_resize_client="$root/target/debug/qemu-authenticated-resize-client"
snap_client="$root/target/debug/qemu-super-snap-client"
edge_client="$root/target/debug/qemu-edge-drop-client"
[[ -x "$edge_client" ]] || deny 'build real XDG edge drop QEMU client'
[[ -x "$snap_client" ]] || deny 'build actual Super Snap QEMU client'
[[ -x "$auth_resize_client" ]] || deny 'build real client xdg_toplevel.resize QEMU probe'
[[ -f "$root/scripts/inject-qemu-alt-tab.py" ]] ||
  deny 'missing USB Alt+Tab focus/KMS QMP acceptance verifier'
[[ -f "$root/scripts/inject-qemu-hover-focus.py" ]] ||
  deny 'missing real QMP tablet hover keyboard-focus verifier'
[[ -f "$root/scripts/inject-qemu-native-tab-stacks.py" ]] ||
  deny 'missing dedicated real USB Wayland tab-stack pixel verifier'
[[ -f "$root/scripts/inject-qemu-combined-tab-tiling.py" ]] ||
  deny 'missing dedicated real QMP grouped XDG tiling verifier'
[[ -f "$root/scripts/inject-qemu-edge-drop.py" ]] ||
  deny 'real tablet edge drop verifier missing'
[[ -f "$root/scripts/inject-qemu-super-snap.py" ]] ||
  deny 'real USB Super Snap QMP CRTC verifier missing'
[[ -f "$root/scripts/inject-qemu-authenticated-resize.py" ]] ||
  deny 'missing real XDG resize and QMP screenshot verifier'
layer_client="$root/target/debug/qemu-layer-panel-client"
[[ -x "$layer_client" ]] || deny 'QEMU real layer-shell client missing'
gtk_five_client="$root/target/debug/qemu-gtk-five-roles"
[[ -x "$gtk_five_client" ]] || deny 'QEMU five GTK namespace probe missing'
# Stage83C: stage the *actual* GTK panel ELF and pinned runtime library
# inside an immutable ISO; this is only a guest linker check, not GTK UI.
gtk_panel="$root/target/debug/melania-panel"
gtk_layer_lib="$root/target/debug/melania-gtk4-layer-shell.so.0"
[[ -x "$gtk_panel" && -s "$gtk_layer_lib" ]] ||
  deny 'Stage83C GTK panel ELF or verified layer-shell SONAME missing'
[[ -x "$two_continuous_client" ]] ||
  deny 'build real two-window continuous Wayland client first'
[[ -x "$continuous_kms" && -x "$continuous_client" ]] ||
  deny 'build continuous DRM recipient and Wayland client first'
[[ -x "$stream_kms" ]] || deny 'build live Rust KMS consumer first'
[[ -f "$root/scripts/capture-qemu-live-kms-stream.py" ]] ||
  deny 'QMP live stream screenshot verifier missing'
rust_kms="$root/target/debug/qemu-rust-kms-frames"
[[ -x "$rust_kms" ]] || deny 'build Rust DRM KMS page-flip executable first'
[[ -f "$root/scripts/capture-qemu-rust-kms-frames.py" ]] ||
  deny 'real two-frame QMP capture verifier missing'
[[ -f "$root/scripts/capture-qemu-two-windows.py" ]] ||
  deny 'real two-window QMP capture verifier missing'
pixel_scanout="$root/target/debug/melania-qemu-client-window-scanout"
[[ -x "$pixel_scanout" ]] || deny 'build QEMU-only real-client scanout program first'
[[ -f "$root/scripts/capture-qemu-client-window.py" ]] ||
  deny 'real client QMP screenshot validator missing'
client_probe="$root/target/debug/melania-qemu-wayland-client"
[[ -f "$client_probe" && -x "$client_probe" ]] ||
  deny 'build real libwayland guest client first'
[[ -f "$root/scripts/inject-qemu-native-input.py" ]] ||
  deny 'QMP virtual USB event injector missing'
[[ -f "$root/scripts/inject-qemu-two-window-focus.py" ]] ||
  deny 'QMP two-window real focus injector missing'
[[ -f "$root/scripts/inject-qemu-window-drag.py" ]] ||
  deny 'QEMU real drag injector missing'
[[ -f "$root/scripts/capture-qemu-drag-frame.py" ]] ||
  deny 'QEMU fourth-frame screendump verifier missing'
[[ -f "$root/scripts/capture-qemu-close-frame.py" ]] ||
  deny 'QEMU fifth-frame close screendump verifier missing'
[[ -f "$root/scripts/capture-qemu-reopen-frame.py" ]] ||
  deny 'QEMU sixth-frame reopen screendump verifier missing'
[[ -f "$root/scripts/capture-qemu-continuous-frames.py" ]] ||
  deny 'QEMU continuous frame7/12 screenshot verifier missing'
[[ -f "$root/scripts/capture-qemu-continuous-two-windows.py" ]] ||
  deny 'QEMU real two-window continuous pixel verifier missing'
[[ -f "$root/scripts/capture-qemu-continuous-reopen.py" ]] ||
  deny 'QEMU real close/reopen frame13/14 screenshot verifier missing'
[[ -f "$root/scripts/capture-qemu-real-layer-panel.py" ]] ||
  deny 'independent QMP layer pixel verifier missing'
[[ -f "$root/scripts/capture-qemu-real-gtk-panel.py" ]] ||
  deny 'independent QMP real GTK4 taskbar verifier missing'
[[ -f "$root/scripts/export-qemu-real-gtk-png.py" ]] ||
  deny 'lossless QMP GTK screenshot PNG encoder missing'
[[ -f "$root/scripts/capture-qemu-continuous-dynamic.py" ]] ||
  deny 'QEMU real variable-size two-window pixel verifier missing'
[[ -f "$root/scripts/inject-qemu-dynamic-pointer.py" &&
   -f "$root/scripts/capture-qemu-dynamic-pointer.py" ]] ||
  deny 'QEMU resized-client real tablet drag proof missing'
[[ -f "$root/scripts/capture-qemu-managed-xdg-roles.py" ]] ||
  deny 'QMP xdg maximize/fullscreen real framebuffer verifier missing'
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
# QEMU-only real Fluent artwork, built from the approved SVG generators.
# In CI use the fresh, verified guest-only theme outside Cargo's restored
# target cache. The default remains available for manual QEMU invocations.
fluent_icons="${MELANIA_QEMU_FLUENT_ICONS_DIR:-$root/target/melania-qemu-fluent-icons}"
[[ -f "$fluent_icons/Melania-Fluent-Light/index.theme" &&
   -f "$fluent_icons/Melania-Fluent-Light/apps/scalable/melania.svg" &&
   -f "$fluent_icons/Melania-Fluent-Light/apps/scalable/org.melaniadesktop.Files.svg" ]] ||
  deny 'genuine Melania Fluent theme must be built before assembling VM ISO'
genisoimage -quiet -R -J -V MELANIA_BIN -o melania-payload.iso \
  -graft-points "melania-native=$native" "melania-vt-guest.py=$root/scripts/melania-vt-guest.py" "melania-fb2-probe=$fb_probe" "melania-visible-probe=$visible_probe" "melania-wayland-client=$client_probe" "melania-shell-client=$shell_probe" "melania-pixel-client=$pixel_client" "melania-pixel-scanout=$pixel_scanout" "melania-rust-kms=$rust_kms" "melania-rust-stream=$stream_kms" "melania-continuous-kms=$continuous_kms" "melania-continuous-client=$continuous_client" "melania-two-continuous-client=$two_continuous_client" "melania-ime-roundtrip=$ime_probe" "melania-auth-resize-client=$auth_resize_client" "melania-real-popup-client=$real_popup_client" "melania-super-snap-client=$snap_client" "melania-edge-drop-client=$edge_client" "melania-layer-panel-client=$layer_client" "melania-gtk-five-roles=$gtk_five_client" "melania-panel=$gtk_panel" "libgtk4-layer-shell.so.0=$gtk_layer_lib" "icons=$fluent_icons"
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
  - libgtk-4-1
  - gsettings-desktop-schemas
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
  --output "$report/visible-frame.ppm" --timeout 1350 \
  > "$report/qmp-scanout.log" 2>&1 &
capture_pid=$!
python3 "$root/scripts/inject-qemu-native-input.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" --timeout 1350 \
  > "$report/qmp-input.log" 2>&1 &
input_pid=$!
python3 "$root/scripts/capture-qemu-client-window.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --output "$report/client-window.ppm" --timeout 1350 \
  > "$report/qmp-client-window.log" 2>&1 &
client_capture_pid=$!
python3 "$root/scripts/capture-qemu-rust-kms-frames.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --first "$report/rust-client-frame1.ppm" \
  --second "$report/rust-client-frame2.ppm" --timeout 1350 \
  > "$report/qmp-rust-frames.log" 2>&1 &
rust_capture_pid=$!
python3 "$root/scripts/capture-qemu-live-kms-stream.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --first "$report/live-client-frame1.ppm" \
  --second "$report/live-client-frame2.ppm" \
  --third "$report/live-client-frame3.ppm" --timeout 1350 \
  > "$report/qmp-live-stream.log" 2>&1 &
live_capture_pid=$!
python3 "$root/scripts/capture-qemu-two-windows.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --first "$report/two-window-frame1.ppm" \
  --second "$report/two-window-frame2.ppm" \
  --third "$report/two-window-frame3.ppm" --timeout 1350 \
  > "$report/qmp-two-windows.log" 2>&1 &
multi_capture_pid=$!
python3 "$root/scripts/inject-qemu-two-window-focus.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" --timeout 1350 \
  > "$report/qmp-two-window-focus.log" 2>&1 &
focus_inject_pid=$!
python3 "$root/scripts/inject-qemu-window-drag.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" --timeout 1350 \
  > "$report/qmp-window-drag.log" 2>&1 &
drag_inject_pid=$!
python3 "$root/scripts/capture-qemu-drag-frame.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --output "$report/drag-window-frame4.ppm" --timeout 1350 \
  > "$report/qmp-drag-frame.log" 2>&1 &
drag_capture_pid=$!
python3 "$root/scripts/capture-qemu-close-frame.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --output "$report/close-window-frame5.ppm" --timeout 1350 \
  > "$report/qmp-close-frame.log" 2>&1 &
close_capture_pid=$!
python3 "$root/scripts/capture-qemu-reopen-frame.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --output "$report/reopen-window-frame6.ppm" --timeout 1350 \
  > "$report/qmp-reopen-frame.log" 2>&1 &
reopen_capture_pid=$!
python3 "$root/scripts/capture-qemu-continuous-frames.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --seventh "$report/continuous-frame7.ppm" \
  --twelfth "$report/continuous-frame12.ppm" --timeout 1350 \
  > "$report/qmp-continuous-frames.log" 2>&1 &
continuous_capture_pid=$!
python3 "$root/scripts/capture-qemu-continuous-two-windows.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --output "$report/continuous-two-windows-frame12.ppm" --timeout 1350 \
  > "$report/qmp-continuous-two-windows.log" 2>&1 &
two_continuous_capture_pid=$!
python3 "$root/scripts/capture-qemu-continuous-reopen.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --close "$report/continuous-close-frame13.ppm" \
  --reopen "$report/continuous-reopen-frame14.ppm" --timeout 1900 \
  > "$report/qmp-continuous-reopen.log" 2>&1 &
continuous_reopen_capture_pid=$!
python3 "$root/scripts/capture-qemu-continuous-dynamic.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --output "$report/dynamic-window-frame12.ppm" --timeout 1680 \
  > "$report/qmp-continuous-dynamic.log" 2>&1 &
dynamic_capture_pid=$!
python3 "$root/scripts/inject-qemu-dynamic-pointer.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" --timeout 1680 \
  > "$report/qmp-dynamic-pointer-input.log" 2>&1 &
dynamic_pointer_input_pid=$!
python3 "$root/scripts/capture-qemu-dynamic-pointer.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --output "$report/dynamic-pointer-frame13.ppm" --timeout 1680 \
  > "$report/qmp-dynamic-pointer-frame13.log" 2>&1 &
dynamic_pointer_capture_pid=$!
python3 "$root/scripts/capture-qemu-real-xdg-popup.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --popup "$report/real-popup-frame2.ppm" \
  --cleared "$report/real-popup-cleared-frame3.ppm" \
  --timeout 1350 > "$report/qmp-real-xdg-popup.log" 2>&1 &
real_popup_capture_pid=$!
python3 "$root/scripts/inject-qemu-alt-tab.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --output "$report/alt-tab-frame13.ppm" --timeout 1350 \
  > "$report/qmp-alt-tab.log" 2>&1 &
alt_tab_input_pid=$!
python3 "$root/scripts/inject-qemu-hover-focus.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --output "$report/hover-focus-frame12.ppm" --timeout 1350 \
  > "$report/qmp-hover-focus.log" 2>&1 &
hover_focus_input_pid=$!
python3 "$root/scripts/inject-qemu-native-tab-stacks.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --out-dir "$report" --timeout 1680 \
  > "$report/qmp-native-tab-stacks.log" 2>&1 &
native_tabs_input_pid=$!
python3 "$root/scripts/inject-qemu-combined-tab-tiling.py"   --serial "$serial" --qmp "$work/melania-qmp.sock"   --out-dir "$report" --timeout 1900   > "$report/qmp-combined-tab-tiling.log" 2>&1 &
combined_tabs_input_pid=$!
python3 "$root/scripts/inject-qemu-authenticated-resize.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --output "$report/authenticated-resize-frame2.ppm" --timeout 1350 \
  > "$report/qmp-authenticated-resize.log" 2>&1 &
auth_resize_inject_pid=$!
python3 "$root/scripts/inject-qemu-super-snap.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --out-dir "$report" --timeout 1350 \
  > "$report/qmp-super-snap.log" 2>&1 &
snap_inject_pid=$!
python3 "$root/scripts/inject-qemu-edge-drop.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --out-dir "$report" --timeout 1350 \
  > "$report/qmp-edge-drop.log" 2>&1 &
edge_inject_pid=$!
# Managed XDG roles run late in the genuine TCG guest (~1400 s).
# Preserve all six independent QMP pixel checks; only extend the observer
# window beyond the actual live frame stage and below QEMU's 1750s hard stop.
python3 "$root/scripts/capture-qemu-managed-xdg-roles.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --out-dir "$report" --timeout 1680 \
  > "$report/qmp-managed-xdg-roles.log" 2>&1 &
managed_role_capture_pid=$!
# The real layer-shell frame 12 is produced after the managed XDG stage:
# its QMP observer must survive the same late TCG boot (~1400s in evidence).
# Keep the exact framebuffer pixel check and stay under QEMU's 1750s limit.
python3 "$root/scripts/capture-qemu-real-layer-panel.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --output "$report/real-layer-panel-frame12.ppm" --timeout 1680 \
  > "$report/qmp-real-layer-panel.log" 2>&1 &
layer_capture_pid=$!
python3 "$root/scripts/capture-qemu-real-gtk-panel.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --output "$report/real-gtk-panel-frame1.ppm" --timeout 1350 \
  > "$report/qmp-real-gtk-panel.log" 2>&1 &
gtk_capture_pid=$!
python3 "$root/scripts/capture-qemu-real-gtk-two-frames.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --first "$report/real-gtk-continuous-frame1.ppm" \
  --second "$report/real-gtk-continuous-frame2.ppm" --timeout 1350 \
  > "$report/qmp-real-gtk-two-frames.log" 2>&1 &
gtk_two_capture_pid=$!
python3 "$root/scripts/capture-qemu-real-gtk-start-overlay.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" \
  --second "$report/real-gtk-continuous-frame2.ppm" \
  --output "$report/real-gtk-start-overlay-frame3.ppm" --timeout 1350 \
  > "$report/qmp-real-gtk-start-overlay.log" 2>&1 &
gtk_start_capture_pid=$!
python3 "$root/scripts/inject-qemu-real-gtk-super-start.py" \
  --serial "$serial" --qmp "$work/melania-qmp.sock" --timeout 1350 \
  > "$report/qmp-real-gtk-super-input.log" 2>&1 &
gtk_super_input_pid=$!
set +e
# Extra *independent* GPU-client/DRM KMS session now runs LAST, after all
# legacy live QMP observers. Give the guest a bounded +300s for that extra
# real 12-frame acceptance; no bypass of any CRTC/pixel/rollback assertion.
timeout --signal=TERM --kill-after=10s 2050s \
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
wait "$multi_capture_pid"
multi_capture_code=$?
wait "$focus_inject_pid"
focus_inject_code=$?
wait "$drag_inject_pid"
drag_inject_code=$?
wait "$drag_capture_pid"
drag_capture_code=$?
wait "$close_capture_pid"
close_capture_code=$?
wait "$reopen_capture_pid"
reopen_capture_code=$?
wait "$continuous_capture_pid"
continuous_capture_code=$?
wait "$two_continuous_capture_pid"
two_continuous_capture_code=$?
wait "$continuous_reopen_capture_pid"
continuous_reopen_capture_code=$?
wait "$dynamic_capture_pid"
dynamic_capture_code=$?
wait "$dynamic_pointer_input_pid"
dynamic_pointer_input_code=$?
wait "$dynamic_pointer_capture_pid"
dynamic_pointer_capture_code=$?
wait "$managed_role_capture_pid"
managed_role_capture_code=$?
wait "$auth_resize_inject_pid"
auth_resize_inject_code=$?
wait "$real_popup_capture_pid"
real_popup_capture_code=$?
wait "$snap_inject_pid"
snap_inject_code=$?
wait "$edge_inject_pid"
edge_inject_code=$?
wait "$alt_tab_input_pid"
alt_tab_input_code=$?
wait "$hover_focus_input_pid"
hover_focus_input_code=$?
wait "$native_tabs_input_pid"
native_tabs_input_code=$?
wait "$combined_tabs_input_pid"
combined_tabs_input_code=$?
wait "$layer_capture_pid"
layer_capture_code=$?
wait "$gtk_capture_pid"
gtk_capture_code=$?
wait "$gtk_two_capture_pid"
gtk_two_capture_code=$?
wait "$gtk_start_capture_pid"
gtk_start_capture_code=$?
wait "$gtk_super_input_pid"
gtk_super_input_code=$?
set -e
printf 'QEMU_EXIT=%s\n' "$qemu_code" > "$report/result.txt"
if [[ "$qemu_code" -ne 0 || "$capture_code" -ne 0 || "$input_code" -ne 0 || "$client_capture_code" -ne 0 || "$rust_capture_code" -ne 0 || "$live_capture_code" -ne 0 || "$multi_capture_code" -ne 0 || "$focus_inject_code" -ne 0 || "$drag_inject_code" -ne 0 || "$drag_capture_code" -ne 0 || "$close_capture_code" -ne 0 || "$reopen_capture_code" -ne 0 || "$continuous_capture_code" -ne 0 || "$two_continuous_capture_code" -ne 0 || "$continuous_reopen_capture_code" -ne 0 || "$dynamic_capture_code" -ne 0 || "$dynamic_pointer_input_code" -ne 0 || "$dynamic_pointer_capture_code" -ne 0 || "$managed_role_capture_code" -ne 0 || "$auth_resize_inject_code" -ne 0 || "$real_popup_capture_code" -ne 0 || "$snap_inject_code" -ne 0 || "$edge_inject_code" -ne 0 || "$alt_tab_input_code" -ne 0 || "$hover_focus_input_code" -ne 0 || "$native_tabs_input_code" -ne 0 || "$combined_tabs_input_code" -ne 0 || "$layer_capture_code" -ne 0 || "$gtk_capture_code" -ne 0 || "$gtk_two_capture_code" -ne 0 || "$gtk_start_capture_code" -ne 0 || "$gtk_super_input_code" -ne 0 ]] ||
    ! grep -q '^MELANIA_QEMU_GTK4_PANEL_PIXELS_FROM_KMS=true' "$report/qmp-real-gtk-panel.log" ||
    ! grep -q '^MELANIA_QEMU_GTK4_TWO_DISTINCT_REAL_FRAMES=true' "$report/qmp-real-gtk-two-frames.log" ||
    ! grep -q '^MELANIA_QEMU_GTK_START_REAL_CRTC_PIXELS=true' "$report/qmp-real-gtk-start-overlay.log" ||
    ! grep -q '^MELANIA_QEMU_GTK_REAL_SUPER_INPUT_PASS=true' "$report/qmp-real-gtk-super-input.log" ||
    ! grep -q '^MELANIA_QEMU_GTK_USB_SEARCH_TEXT_SENT=true' "$report/qmp-real-gtk-super-input.log" ||
    ! test -s "$report/real-gtk-start-overlay-frame3.ppm" ||
    ! grep -q '^MELANIA_QEMU_REAL_GTK_START_OVERLAY_KMS_SCANOUT_PASS=true' "$serial" ||
    ! test -s "$report/real-gtk-continuous-frame1.ppm" ||
    ! test -s "$report/real-gtk-continuous-frame2.ppm" ||
    ! grep -q '^MELANIA_QEMU_REAL_GTK_TWO_FRAME_KMS_SCANOUT_PASS=true' "$serial" ||
    ! test -s "$report/real-gtk-panel-frame1.ppm" ||
    ! grep -q '^MELANIA_QEMU_REAL_LAYER_PANEL_PIXELS=true' "$report/qmp-real-layer-panel.log" ||
    ! grep -q '^MELANIA_QEMU_REAL_LAYER_SHELL_12_KMS_PASS=true' "$serial" ||
    ! test -s "$report/real-layer-panel-frame12.ppm" ||
    ! grep -q '^MELANIA_QEMU_XDG_POPUP_REAL_USB_GRAB=true' "$report/qmp-real-xdg-popup.log" ||
    ! grep -q '^MELANIA_QEMU_XDG_POPUP_REAL_CRTC_PIXELS=true' "$report/qmp-real-xdg-popup.log" ||
    ! grep -q '^MELANIA_QEMU_XDG_POPUP_USB_ESC_POPUP_DONE=true' "$report/qmp-real-xdg-popup.log" ||
    ! grep -q '^MELANIA_QEMU_XDG_POPUP_DESTROY_CRTC_CLEARED=true' "$report/qmp-real-xdg-popup.log" ||
    ! test -s "$report/real-popup-frame2.ppm" ||
    ! test -s "$report/real-popup-cleared-frame3.ppm" ||
    ! grep -q '^MELANIA_QEMU_REAL_XDG_POPUP_KMS3_PASS=true' "$serial" ||
    ! grep -q '^MELANIA_QEMU_TAB_REAL_USB_XDG_FOCUS_CRTC_PIXELS=true' "$report/qmp-native-tab-stacks.log" ||
    ! grep -q '^MELANIA_QEMU_TAB_REAL_CRTC_ATTACHED_A=true' "$report/qmp-native-tab-stacks.log" ||
    ! grep -q '^MELANIA_QEMU_TAB_REAL_CRTC_SELECTED_B=true' "$report/qmp-native-tab-stacks.log" ||
    ! grep -q '^MELANIA_QEMU_TAB_REAL_CRTC_SELECTED_A=true' "$report/qmp-native-tab-stacks.log" ||
    ! grep -q '^MELANIA_QEMU_TAB_REAL_CRTC_DISSOLVED=true' "$report/qmp-native-tab-stacks.log" ||
    ! test -s "$report/tab-stack-before.ppm" ||
    ! test -s "$report/tab-stack-attached-A.ppm" ||
    ! test -s "$report/tab-stack-B-active.ppm" ||
    ! test -s "$report/tab-stack-A-restored.ppm" ||
    ! test -s "$report/tab-stack-dissolved.ppm" ||
    ! grep -q '^MELANIA_QEMU_NATIVE_REAL_TAB_STACK_USB_KMS_PASS=true' "$serial" ||
    ! grep -q '^MELANIA_QEMU_COMBINED_TAB_REAL_XDG_SHM_KMS_PASS=true' "$report/qmp-combined-tab-tiling.log" ||
    ! grep -q '^MELANIA_QEMU_COMBINED_TAB_ATOMIC_CRTC_A=true' "$report/qmp-combined-tab-tiling.log" ||
    ! grep -q '^MELANIA_QEMU_COMBINED_TAB_REAL_CRTC_B=true' "$report/qmp-combined-tab-tiling.log" ||
    ! grep -q '^MELANIA_QEMU_COMBINED_TAB_REAL_CRTC_A_RESTORED=true' "$report/qmp-combined-tab-tiling.log" ||
    ! grep -q '^MELANIA_QEMU_NATIVE_COMBINED_TAB_TILING_REAL_CRTC_PASS=true' "$serial" ||
    ! test -s "$report/combined-tabs-before.ppm" ||
    ! test -s "$report/combined-tabs-active-A.ppm" ||
    ! test -s "$report/combined-tabs-active-B.ppm" ||
    ! test -s "$report/combined-tabs-active-A-return.ppm" ||
    ! grep -q '^MELANIA_QEMU_ALT_TAB_REAL_WL_KEYBOARD_ENTER_A_B=true' "$report/qmp-alt-tab.log" ||
    ! grep -q '^MELANIA_QEMU_ALT_F4_REAL_WAYLAND_CLOSE=true' "$report/qmp-alt-tab.log" ||
    ! grep -q '^MELANIA_QEMU_ALT_TAB_REAL_KERNEL_CRTC_FRAME13_PIXELS=true' "$report/qmp-alt-tab.log" ||
    ! test -s "$report/alt-tab-frame13.ppm" ||
    ! grep -q '^MELANIA_QEMU_REAL_USB_ALT_TAB_WAYLAND_KMS_PASS=true' "$serial" ||
    ! grep -q '^MELANIA_QEMU_HOVER_QMP_NO_CLICK_PASS=true' "$report/qmp-hover-focus.log" ||
    ! grep -q '^MELANIA_QEMU_HOVER_CRTC12_REAL_CLIENT_PIXELS=true' "$report/qmp-hover-focus.log" ||
    ! test -s "$report/hover-focus-frame12.ppm" ||
    ! grep -q '^MELANIA_QEMU_HOVER_FOCUS_REAL_WAYLAND_PASS=true' "$serial" ||
    ! grep -q '^MELANIA_QEMU_EDGE_DROP_REAL_CRTC_HALF_PIXELS=true' "$report/qmp-edge-drop.log" ||
    ! test -s "$report/edge-drop-original-frame1.ppm" ||
    ! test -s "$report/edge-drop-half-frame2.ppm" ||
    ! grep -q '^MELANIA_QEMU_EDGE_DROP_XDG_REAL_CRTC_PASS=true' "$serial" ||
    ! grep -q '^MELANIA_QEMU_SUPER_SNAP_HALF_CRTC_PIXELS=true' "$report/qmp-super-snap.log" ||
    ! grep -q '^MELANIA_QEMU_SUPER_SNAP_RESTORE_CRTC_PIXELS=true' "$report/qmp-super-snap.log" ||
    ! grep -q '^MELANIA_QEMU_SUPER_SNAP_RIGHT_THIRD_CRTC_PIXELS=true' "$report/qmp-super-snap.log" ||
    ! grep -q '^MELANIA_QEMU_SUPER_SNAP_THIRD_RESTORE_CRTC_PIXELS=true' "$report/qmp-super-snap.log" ||
    ! grep -q '^MELANIA_QEMU_SUPER_SNAP_NORMAL_MINIMIZED_CRTC_PIXELS=true' "$report/qmp-super-snap.log" ||
    ! test -s "$report/super-snap-original-frame1.ppm" ||
    ! test -s "$report/super-snap-half-frame2.ppm" ||
    ! test -s "$report/super-snap-restored-frame3.ppm" ||
    ! test -s "$report/super-snap-right-third-frame4.ppm" ||
    ! test -s "$report/super-snap-third-restored-frame5.ppm" ||
    ! test -s "$report/super-snap-minimized-frame6.ppm" ||
    ! grep -q '^MELANIA_QEMU_SUPER_SNAP_USB_XDG_KMS5_PASS=true' "$serial" ||
    ! grep -q '^MELANIA_QEMU_AUTH_RESIZE_CRTC_PIXELS=true' "$report/qmp-authenticated-resize.log" ||
    ! grep -q '^MELANIA_QEMU_AUTH_RESIZE_REAL_POINTER_RELEASE=true' "$report/qmp-authenticated-resize.log" ||
    ! test -s "$report/authenticated-resize-frame2.ppm" ||
    ! grep -q '^MELANIA_QEMU_AUTHENTICATED_XDG_RESIZE_REAL_KMS_PASS=true' "$serial" ||
    ! grep -q '^MELANIA_QEMU_MANAGED_XDG_FOUR_FRAMES_VERIFIED=true' "$report/qmp-managed-xdg-roles.log" ||
    ! grep -q '^MELANIA_QEMU_MANAGED_XDG_SIX_FRAMES_VERIFIED=true' "$report/qmp-managed-xdg-roles.log" ||
    ! grep -q '^MELANIA_QEMU_MANAGED_XDG_FRAME13_PIXELS_VERIFIED=true' "$report/qmp-managed-xdg-roles.log" ||
    ! grep -q '^MELANIA_QEMU_MANAGED_XDG_FRAME14_PIXELS_VERIFIED=true' "$report/qmp-managed-xdg-roles.log" ||
    ! test -s "$report/managed-role-frame13.ppm" ||
    ! test -s "$report/managed-role-frame14.ppm" ||
    ! grep -q '^MELANIA_QEMU_MANAGED_MINIMIZE_RESTORE_14_KMS_ACKS_PASS=true' "$serial" ||
    ! grep -q '^MELANIA_QEMU_MANAGED_XDG_FRAME3_PIXELS_VERIFIED=true' "$report/qmp-managed-xdg-roles.log" ||
    ! grep -q '^MELANIA_QEMU_MANAGED_XDG_FRAME5_PIXELS_VERIFIED=true' "$report/qmp-managed-xdg-roles.log" ||
    ! grep -q '^MELANIA_QEMU_MANAGED_XDG_FRAME7_PIXELS_VERIFIED=true' "$report/qmp-managed-xdg-roles.log" ||
    ! grep -q '^MELANIA_QEMU_MANAGED_XDG_FRAME9_PIXELS_VERIFIED=true' "$report/qmp-managed-xdg-roles.log" ||
    ! test -s "$report/managed-role-frame3.ppm" ||
    ! test -s "$report/managed-role-frame5.ppm" ||
    ! test -s "$report/managed-role-frame7.ppm" ||
    ! test -s "$report/managed-role-frame9.ppm" ||
    ! grep -q '^MELANIA_QEMU_MANAGED_XDG_12_REAL_KMS_ACKS_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_DYNAMIC_LIBINPUT_DRAG_FRAME13=true' "$report/qmp-dynamic-pointer-input.log" ||
   ! grep -q '^MELANIA_QEMU_DYNAMIC_POINTER_FRAME13_PIXELS=true' "$report/qmp-dynamic-pointer-frame13.log" ||
   ! test -s "$report/dynamic-pointer-frame13.ppm" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_DYNAMIC_POINTER_KMS13_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_REAL_POINTER_DRAG_TO_KMS=true' "$report/qmp-window-drag.log" ||
   ! grep -q '^MELANIA_QEMU_DRAG_FRAME4_PIXELS_VERIFIED=true' "$report/qmp-drag-frame.log" ||
   ! test -s "$report/drag-window-frame4.ppm" ||
   ! grep -q '^MELANIA_QEMU_CLOSE_FRAME5_PIXELS_VERIFIED=true' "$report/qmp-close-frame.log" ||
   ! test -s "$report/close-window-frame5.ppm" ||
   ! grep -q '^MELANIA_QEMU_REOPEN_FRAME6_PIXELS_VERIFIED=true' "$report/qmp-reopen-frame.log" ||
   ! test -s "$report/reopen-window-frame6.ppm" ||
   ! grep -q '^MELANIA_QEMU_CONTINUOUS_DYNAMIC_PIXELS_VERIFIED=true' "$report/qmp-continuous-dynamic.log" ||
   ! test -s "$report/dynamic-window-frame12.ppm" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_DYNAMIC_TWO_SIZES_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_CONTINUOUS_FRAME13_PIXELS_VERIFIED=true' "$report/qmp-continuous-reopen.log" ||
   ! grep -q '^MELANIA_QEMU_CONTINUOUS_FRAME14_PIXELS_VERIFIED=true' "$report/qmp-continuous-reopen.log" ||
   ! test -s "$report/continuous-close-frame13.ppm" ||
   ! test -s "$report/continuous-reopen-frame14.ppm" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_CONTINUOUS_REOPEN_14_FRAMES_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_CONTINUOUS_TWO_WINDOWS_PIXELS_VERIFIED=true' "$report/qmp-continuous-two-windows.log" ||
   ! test -s "$report/continuous-two-windows-frame12.ppm" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_CONTINUOUS_TWO_WINDOWS_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_CONTINUOUS_FRAME7_PIXELS_VERIFIED=true' "$report/qmp-continuous-frames.log" ||
   ! grep -q '^MELANIA_QEMU_CONTINUOUS_FRAME12_PIXELS_VERIFIED=true' "$report/qmp-continuous-frames.log" ||
   ! test -s "$report/continuous-frame7.ppm" ||
   ! test -s "$report/continuous-frame12.ppm" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_GBM_OUTPUT_SIZED_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_GBM_KMS_TWO_REAL_FRAMES_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_CONTINUOUS_12_REAL_FRAMES_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_IN_PROCESS_WAYLAND_KMS_12_FRAMES_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_IN_PROCESS_WAYLAND_KMS_24_FRAMES_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_REOPEN_FRAME_VISIBLE=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_CLIENT_REOPEN_CALLBACK_AFTER_KMS=true' "$serial" ||
   ! grep -q '^MELANIA_RUST_REOPEN_FRAME6_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_REAL_WINDOW_REOPEN_KMS_FRAME6_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_CLOSE_FRAME_VISIBLE=true' "$serial" ||
   ! grep -q '^MELANIA_RUST_CLOSE_FRAME5_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_REAL_WINDOW_CLOSE_KMS_FRAME5_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_DRAG_FRAME_VISIBLE=true' "$serial" ||
   ! grep -q '^MELANIA_RUST_COMPOSITOR_DRAG_FRAME4_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_TWO_WINDOW_REAL_INPUT_PASS=true' "$report/qmp-two-window-focus.log" ||
   ! grep -q '^MELANIA_QEMU_WINDOW2_POINTER_FOCUS=true' "$report/qmp-two-window-focus.log" ||
   ! grep -q '^MELANIA_QEMU_WINDOW1_POINTER_FOCUS=true' "$report/qmp-two-window-focus.log" ||
   ! grep -q '^MELANIA_QEMU_TWO_WINDOWS_THREE_CAPTURE_PASS=true' "$report/qmp-two-windows.log" ||
   ! grep -q '^MELANIA_QEMU_TWO_WINDOWS_FRAME1_PIXELS=true' "$report/qmp-two-windows.log" ||
   ! grep -q '^MELANIA_QEMU_TWO_WINDOWS_FRAME2_PIXELS=true' "$report/qmp-two-windows.log" ||
   ! grep -q '^MELANIA_QEMU_TWO_WINDOWS_FRAME3_PIXELS=true' "$report/qmp-two-windows.log" ||
   ! test -s "$report/two-window-frame1.ppm" ||
   ! test -s "$report/two-window-frame2.ppm" ||
   ! test -s "$report/two-window-frame3.ppm" ||
   ! grep -q '^MELANIA_QEMU_TWO_REAL_WINDOWS_COMPOSITED_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NONBLOCKING_KMS_WORKER_PASS=true' "$serial" ||
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
   ! grep -q '^MELANIA_QEMU_NATIVE_PRESENTED_FRAME_CALLBACKS_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_CLIENT_FRAME_CALLBACKS_PRESENTED_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_FRAME_CALLBACKS_PRESENTED_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_FRAME_CALLBACK_COUNT=3' "$serial" ||
   ! grep -q '^MELANIA_NATIVE_CLIENT_FRAME_PRESENTED=3' "$serial" ||
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
   ! grep -q '^MELANIA_QEMU_GTK_PANEL_ELF_LINK_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_GTK_FIVE_REAL_LAYER_ROLE_ACK_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_REAL_GTK_PANEL_STARTUP_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_REAL_GTK_PANEL_KMS_SCANOUT_PASS=true' "$serial" ||
   ! grep -q '^MELANIA_QEMU_NATIVE_GUEST_EXIT=0' "$serial"; then
  printf '%s\n' 'MELANIA_QEMU_NATIVE_RESULT=FAIL' | tee -a "$report/result.txt"
  tail -n 100 "$serial" || true
  tail -n 30 "$report/qmp-scanout.log" || true
  tail -n 30 "$report/qmp-input.log" || true
  tail -n 30 "$report/qmp-client-window.log" || true
  tail -n 30 "$report/qmp-rust-frames.log" || true
  tail -n 30 "$report/qmp-live-stream.log" || true
  tail -n 30 "$report/qmp-two-windows.log" || true
  tail -n 30 "$report/qmp-two-window-focus.log" || true
  tail -n 30 "$report/qmp-window-drag.log" || true
  tail -n 30 "$report/qmp-drag-frame.log" || true
  tail -n 30 "$report/qmp-close-frame.log" || true
  tail -n 30 "$report/qmp-reopen-frame.log" || true
  tail -n 30 "$report/qmp-managed-xdg-roles.log" || true
  tail -n 30 "$report/qmp-real-layer-panel.log" || true
  tail -n 30 "$report/qmp-real-gtk-panel.log" || true
  tail -n 30 "$report/qmp-real-gtk-two-frames.log" || true
  tail -n 30 "$report/qmp-real-gtk-start-overlay.log" || true
  tail -n 30 "$report/qmp-real-gtk-super-input.log" || true
  exit 1
fi
# Export only the previously verified LIVE QMP GTK framebuffer, preserving
# the original PPM as forensic evidence. Never synthesize a screenshot.
python3 "$root/scripts/export-qemu-real-gtk-png.py" \
  --ppm "$report/real-gtk-panel-frame1.ppm" \
  --png "$report/real-gtk-panel-frame1.png" \
  | tee "$report/qmp-real-gtk-png.log"
test -s "$report/real-gtk-panel-frame1.png" ||
  deny 'verified real GTK4 screenshot PNG was not written'
# Export only two live-QMP-validated PPMs; retain originals as evidence.
python3 "$root/scripts/export-qemu-real-gtk-png.py" \
  --ppm "$report/real-gtk-continuous-frame1.ppm" \
  --png "$report/real-gtk-continuous-frame1.png" \
  | tee "$report/qmp-real-gtk-continuous-png.log"
python3 "$root/scripts/export-qemu-real-gtk-png.py" \
  --ppm "$report/real-gtk-continuous-frame2.ppm" \
  --png "$report/real-gtk-continuous-frame2.png" \
  | tee -a "$report/qmp-real-gtk-continuous-png.log"
test -s "$report/real-gtk-continuous-frame1.png" &&
  test -s "$report/real-gtk-continuous-frame2.png" ||
  deny 'real GTK4 two-frame PNG evidence missing'
python3 "$root/scripts/export-qemu-real-gtk-png.py" \
  --ppm "$report/real-gtk-start-overlay-frame3.ppm" \
  --png "$report/real-gtk-start-overlay-frame3.png" \
  | tee "$report/qmp-real-gtk-start-png.log"
test -s "$report/real-gtk-start-overlay-frame3.png" ||
  deny 'real GTK Start overlay lossless screenshot PNG missing'
printf '%s\n' 'MELANIA_QEMU_NATIVE_RESULT=PASS' | tee -a "$report/result.txt"
printf '%s\n' 'MELANIA_VM_NATIVE_CONTINUOUS_12_KMS_ACKS=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_NATIVE_CONTINUOUS_FRAME7_12_PIXELS=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_NATIVE_TWO_CONTINUOUS_WINDOWS_QMP=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_NATIVE_CONTINUOUS_CLOSE_REOPEN_14_QMP=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_NATIVE_VARIABLE_CLIENT_SIZES_GPU_QMP=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_NATIVE_MANAGED_XDG_FOUR_QMP_FRAMES=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_NATIVE_MANAGED_MINIMIZE_RESTORE_SIX_QMP_FRAMES=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_NATIVE_REAL_LAYER_SHELL_QMP=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_GTK_FIVE_WAYLAND_ROLE_ACK=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_REAL_GTK_PANEL_STARTUP=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_REAL_GTK_PANEL_KMS_QMP_PIXELS=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_REAL_GTK_PANEL_SCREENSHOT_PNG=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_REAL_GTK_TWO_DIFFERENT_KMS_FRAMES_QMP=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_REAL_GTK_START_MENU_REAL_KMS_QMP=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_GTK_PANEL_REAL_ELF_GUEST_LINK=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_ONLY_VISIBLE_FRAME_AND_CRTC_RESTORE=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_WAYLAND_REAL_INPUT_DELIVERY=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_XDG_LAYER_SHELL_HANDSHAKE=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_CLIENT_PIXELS_RUST_COMPOSED_C_SCANOUT=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_NATIVE_WL_OUTPUT_MODE_VERIFIED=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_RUST_DIRECT_KMS_TWO_CLIENT_FRAMES=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_LIVE_WAYLAND_STREAM_THREE_FRAMES=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_NATIVE_POST_KMS_FRAME_CALLBACKS_VERIFIED=true' >> "$report/result.txt"

printf '%s\n' 'MELANIA_VM_TWO_REAL_WAYLAND_WINDOWS_QMP_PRESENTED=true' >> "$report/result.txt"

printf '%s\n' 'MELANIA_VM_TWO_WINDOWS_REAL_FOCUS_INPUT=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_REAL_POINTER_DRAG_FOURTH_KMS_FRAME=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_REAL_WINDOW_CLOSE_FIFTH_KMS_FRAME=true' >> "$report/result.txt"
printf '%s\n' 'MELANIA_VM_REAL_WINDOW_REOPEN_SIXTH_KMS_FRAME=true' >> "$report/result.txt"
