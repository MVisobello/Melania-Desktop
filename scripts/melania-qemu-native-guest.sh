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
# Stage83C: no GTK app is launched yet; validate only that the exact
# host-compiled GTK panel ELF links to guest GTK4 and the ISO's pinned
# gtk4-layer-shell SONAME. Never mistake this for a native screenshot.
gtk_panel=/mnt/melania-native/melania-panel
gtk_layer_lib=/mnt/melania-native/libgtk4-layer-shell.so.0
# A genuine freedesktop Melania-Fluent-Light theme MUST exist in this VM.
# Avoid silently accepting Adwaita when the real icon pack is missing.
gtk_icons=/mnt/melania-native/icons/Melania-Fluent-Light
[[ -f "$gtk_icons/index.theme" &&
   -f "$gtk_icons/apps/scalable/melania.svg" &&
   -f "$gtk_icons/apps/scalable/org.melaniadesktop.Files.svg" &&
   -f "$gtk_icons/apps/scalable/org.melaniadesktop.Settings.svg" &&
   -f "$gtk_icons/status/24x24/network-wireless-signal-excellent-symbolic.svg" ]] ||
  fatal 'isolated_real_Melania_Fluent_icon_theme_missing'
report 'MELANIA_QEMU_REAL_FLUENT_THEME_BUNDLED=true'
[[ -x "$gtk_panel" && -s "$gtk_layer_lib" ]] ||
  fatal 'real_GTK_panel_or_layer_shell_payload_missing'
gtk_link_log="$(mktemp -t melania-gtk-guest-link-XXXXXX)"
if ! LD_LIBRARY_PATH=/mnt/melania-native ldd "$gtk_panel" > "$gtk_link_log" 2>&1; then
  cat "$gtk_link_log"
  fatal 'real_GTK_panel_ELF_loader_failed_in_guest'
fi
cat "$gtk_link_log"
if grep -q 'not found' "$gtk_link_log" ||
   ! grep -q 'libgtk-4.so.1 => ' "$gtk_link_log" ||
   ! grep -q 'libgtk4-layer-shell.so.0 => /mnt/melania-native/' "$gtk_link_log"; then
  rm -f "$gtk_link_log"
  fatal 'real_GTK_panel_dependency_missing_in_guest'
fi
rm -f "$gtk_link_log"
report 'MELANIA_QEMU_GTK_PANEL_ELF_LINK_PASS=true'
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

# Probe two REAL monitor-sized GBM DMA-BUF/GLES frames, rendered against the
# libseat-owned guest VirtIO GPU. This does NOT change the KMS scanout.
gpu_output_log="$(mktemp -t melania-gbm-output-XXXXXX)"
if ! timeout --signal=TERM --kill-after=3s 45s \
  "$binary" --experimental-gbm-output < /dev/tty1 | tee "$gpu_output_log"; then
  cat "$gpu_output_log"
  fatal 'native_output_sized_GBM_GLES_rendering_failed'
fi
if ! grep -Eq '^MELANIA_NATIVE_GBM_OUTPUT_SIZED_SYNCHRONIZED=[0-9]+x[0-9]+x2$' "$gpu_output_log"; then
  cat "$gpu_output_log"
  fatal 'native_output_sized_GBM_two_frames_not_verified'
fi
rm -f "$gpu_output_log"
report 'MELANIA_QEMU_NATIVE_GBM_OUTPUT_SIZED_PASS=true'

# Real GPU-to-DRM bridge, ONLY inside this disposable QEMU VM. Unlike the
# preceding renderer-only probe this imports two GLES-rendered DMA-BUFs
# as DRM framebuffer IDs, verifies both KMS scanouts and restores the
# original Linux console CRTC/connector before libseat releases the card.
gpu_kms_log="$(mktemp -t melania-gbm-kms-XXXXXX)"
if ! MELANIA_NATIVE_VM_GBM_KMS=YES_DISPOSABLE_QEMU \
  timeout --signal=TERM --kill-after=3s 55s \
  "$binary" --experimental-gbm-kms-scanout < /dev/tty1 \
  > "$gpu_kms_log" 2>&1; then
  cat "$gpu_kms_log"
  fatal 'native_GLES_DMA_BUF_to_KMS_scanout_failed'
fi
cat "$gpu_kms_log"
for marker in \
  'MELANIA_NATIVE_GBM_KMS_GPU_FRAME_VISIBLE=1' \
  'MELANIA_NATIVE_GBM_KMS_GPU_FRAME_VISIBLE=2' \
  'MELANIA_NATIVE_GBM_KMS_VBLANK_EVENT_ACK=2' \
  'MELANIA_NATIVE_GBM_KMS_TOPOLOGY_STABLE_OUTPUTS=1' \
  'MELANIA_NATIVE_GBM_KMS_FLIP_LEDGER_CONFIRMED=1' \
  'MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true' \
  'MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true' \
  'MELANIA_NATIVE_GBM_KMS_TWO_GPU_FRAMES_PASS=true' \
  'MELANIA_NATIVE_GBM_KMS_LIBSEAT_RELEASED=true'; do
  grep -q "^${marker}$" "$gpu_kms_log" ||
    fatal "native_GLES_KMS_missing_${marker}"
done
rm -f "$gpu_kms_log"
report 'MELANIA_QEMU_NATIVE_GBM_KMS_TWO_REAL_FRAMES_PASS=true'

# Stage108: prove that the same GPU/GBM KMS path reuses exactly two
# output-sized DMA-BUFs for 24 real GLES renders and kernel page flips.
# Every flip must be acknowledged by the kernel and validated on the CRTC
# before its buffer is available for the next frame. This is NOT a login.
gpu_stress_log="$(mktemp -t melania-gbm-stress-XXXXXX)"
if ! MELANIA_NATIVE_VM_GBM_KMS=YES_DISPOSABLE_QEMU \
     MELANIA_NATIVE_VM_GBM_STRESS=YES_DISPOSABLE_QEMU \
    timeout --signal=TERM --kill-after=3s 90s \
    "$binary" --experimental-gbm-kms-scanout < /dev/tty1 \
    > "$gpu_stress_log" 2>&1; then
  cat "$gpu_stress_log"
  fatal 'native_24_GLES_DMA_BUF_KMS_scanouts_failed'
fi
for index in $(seq 1 24); do
  grep -q "^MELANIA_NATIVE_GBM_KMS_GPU_FRAME_VISIBLE=$index$" "$gpu_stress_log" ||
    fatal "GPU_stress_real_CRTC_frame_${index}_missing"
done
for marker in \
  'MELANIA_NATIVE_GBM_KMS_TOPOLOGY_STABLE_OUTPUTS=1' \
  'MELANIA_NATIVE_GBM_KMS_VBLANK_EVENT_ACK=24' \
  'MELANIA_NATIVE_GBM_KMS_FLIP_LEDGER_CONFIRMED=23' \
  'MELANIA_NATIVE_GBM_KMS_REUSED_DMA_BUFS=2' \
  'MELANIA_NATIVE_GBM_KMS_24_GPU_FRAMES_PASS=true' \
  'MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true' \
  'MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true' \
  'MELANIA_NATIVE_GBM_KMS_LIBSEAT_RELEASED=true'; do
  grep -q "^$marker$" "$gpu_stress_log" || {
    cat "$gpu_stress_log"
    fatal "GPU_stress_missing_${marker}"
  }
done
rm -f "$gpu_stress_log"
report 'MELANIA_QEMU_NATIVE_GBM_KMS_24_GPU_FRAMES_PASS=true'

# Verify live old framebuffer metadata and acquired GEM handle lifecycle.
# This audit opens DRM only INSIDE the disposable QEMU guest and calls
# drmModeGetFB2 then GEM_CLOSE for each distinct returned GEM handle.
# No plane/mode/atomic setter is issued. It is not a rollback implementation.
fb2_probe=/mnt/melania-native/melania-fb2-probe
[[ -f "$fb2_probe" && -x "$fb2_probe" ]] ||
  fatal 'GEM_lease_probe_missing_from_payload'
MELANIA_FB2_QEMU_ONLY=YES_DISPOSABLE_VM \
  timeout --signal=TERM --kill-after=3s 20s "$fb2_probe" < /dev/tty1 ||
  fatal 'framebuffer_GEM_lifecycle_unverified'
report 'MELANIA_QEMU_NATIVE_FB2_GEM_CLOSE_PASS=true'

# Stage 23: a single visible framebuffer in a DISPOSABLE VM, separate from
# melania-native's still-disabled compositor scanout. This probe captures
# the old CRTC FB+mode, sets a synthetic VM-only framebuffer, and MUST restore
# the exact original CRTC before exiting. The QMP host watcher captures
# real virtual display pixels while the new framebuffer is active.
visible_probe=/mnt/melania-native/melania-visible-probe
[[ -f "$visible_probe" && -x "$visible_probe" ]] ||
  fatal 'QEMU_only_visible_probe_missing_from_payload'
MELANIA_QEMU_VISIBLE_FRAME=YES_DISPOSABLE_QEMU \
  timeout --signal=TERM --kill-after=3s 22s "$visible_probe" < /dev/tty1 ||
  fatal 'QEMU_visible_frame_or_CRTC_restore_failed'
report 'MELANIA_QEMU_NATIVE_VISIBLE_EXPERIMENT_PASS=true'

# Only AFTER the native process returns, confirm the console can still be
# changed and restored via Linux VT ioctl in the SAME disposable guest.
# This is a real VT test but NOT a rollback after visible scanout.
vt_probe=/mnt/melania-native/melania-vt-guest.py
[[ -f "$vt_probe" ]] || fatal 'vt_recovery_probe_missing_from_payload'
timeout --signal=TERM --kill-after=3s 25s python3 "$vt_probe" ||
  fatal 'post_KMS_linux_VT_recovery_failed'
report 'MELANIA_QEMU_NATIVE_POST_KMS_VT_PASS=true'
# Stage24: real Rust Smithay wl_compositor/wl_seat on the dedicated VM VT.
# This is a headless protocol/input bridge: no client framebuffer is scanned
# out and it MUST NOT install a login session on the guest or host.
client_probe=/mnt/melania-native/melania-wayland-client
[[ -f "$client_probe" && -x "$client_probe" ]] ||
  fatal 'real_libwayland_client_probe_missing'
bridge_log="$(mktemp -t melania-wayland-bridge-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 32s \
  "$binary" --experimental-wayland-seat < /dev/tty1 > "$bridge_log" 2>&1 &
bridge_pid=$!
bridge_socket=''
for attempt in $(seq 1 100); do
  bridge_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$bridge_log" | head -n 1)"
  [[ -n "$bridge_socket" ]] && break
  kill -0 "$bridge_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$bridge_socket" ]]; then
  cat "$bridge_log"
  kill "$bridge_pid" 2>/dev/null || true
  wait "$bridge_pid" 2>/dev/null || true
  fatal 'native_Wayland_bridge_socket_unavailable'
fi
report 'MELANIA_QEMU_NATIVE_WAYLAND_BRIDGE_STARTED=true'
if ! WAYLAND_DISPLAY="$bridge_socket" \
  timeout --signal=TERM --kill-after=3s 19s "$client_probe"; then
  cat "$bridge_log"
  kill "$bridge_pid" 2>/dev/null || true
  wait "$bridge_pid" 2>/dev/null || true
  fatal 'real_Wayland_keyboard_pointer_delivery_failed'
fi
if ! wait "$bridge_pid"; then
  cat "$bridge_log"
  fatal 'native_Wayland_bridge_failed'
fi
cat "$bridge_log"
grep -q '^MELANIA_NATIVE_WAYLAND_INPUT_FORWARDED=true' "$bridge_log" ||
  fatal 'libinput_events_not_forwarded_to_Smithay'
rm -f "$bridge_log"
report 'MELANIA_QEMU_NATIVE_WAYLAND_INPUT_PASS=true'
report 'MELANIA_QEMU_NATIVE_WAYLAND_RENDER_READY=false'

# Stage 25: same Rust Wayland server, but exercise actual xdg-shell window and
# wlr-layer-shell panel configure/ack with a second client in a second,
# disposable VM-only run. This is a PROTOCOL milestone, not a visible GTK UI.
shell_client=/mnt/melania-native/melania-shell-client
[[ -x "$shell_client" ]] || fatal 'real_xdg_layer_protocol_client_missing'
shell_log="$(mktemp -t melania-wayland-shell-XXXXXX)"
shell_client_log="$(mktemp -t melania-wayland-role-client-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM \
MELANIA_NATIVE_SHELL_PROBE=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 32s \
  "$binary" --experimental-wayland-seat < /dev/tty1 > "$shell_log" 2>&1 &
shell_pid=$!
shell_socket=''
for attempt in $(seq 1 100); do
  shell_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$shell_log" | head -n 1)"
  [[ -n "$shell_socket" ]] && break
  kill -0 "$shell_pid" 2>/dev/null || break
  sleep .1
done
shell_control_socket="$(sed -n 's/^MELANIA_NATIVE_CONTROL_SOCKET_READY=//p' "$shell_log" | head -n 1)"
if [[ -z "$shell_socket" || -z "$shell_control_socket" || ! -S "$shell_control_socket" ]]; then
  cat "$shell_log"
  kill "$shell_pid" 2>/dev/null || true
  wait "$shell_pid" 2>/dev/null || true
  fatal 'native_xdg_layer_socket_unavailable'
fi
if ! MELANIA_CONTROL_SOCKET="$shell_control_socket" \
     MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM WAYLAND_DISPLAY="$shell_socket" \
  timeout --signal=TERM --kill-after=3s 19s "$shell_client" > "$shell_client_log" 2>&1; then
  cat "$shell_client_log" "$shell_log"
  kill "$shell_pid" 2>/dev/null || true
  wait "$shell_pid" 2>/dev/null || true
  fatal 'real_xdg_layer_configure_ack_failed'
fi
if ! wait "$shell_pid"; then
  cat "$shell_log"
  fatal 'native_wayland_shell_server_failed'
fi
cat "$shell_client_log" "$shell_log"
grep -q '^MELANIA_NATIVE_XDG_ROLE_ROUNDTRIP_PASS=true' "$shell_client_log" ||
  fatal 'real_xdg_maximize_fullscreen_client_ACK_missing'
grep -q '^MELANIA_NATIVE_XDG_MINIMIZE_REQUEST_SENT=true' "$shell_client_log" ||
  fatal 'real_xdg_minimize_request_missing'
grep -q '^MELANIA_NATIVE_MANAGED_XDG_MINIMIZED=true' "$shell_log" ||
  fatal 'real_xdg_minimize_not_delivered'
grep -q '^MELANIA_NATIVE_CONTROL_RESTORE_PASS=true' "$shell_client_log" ||
  fatal 'real_xdg_restore_IPC_failed'
grep -q '^MELANIA_NATIVE_CONTROL_ACTIVATED=1' "$shell_log" ||
  fatal 'real_xdg_restore_not_applied'
for role_event in \
  'MELANIA_NATIVE_MANAGED_XDG_MAXIMIZED=true' \
  'MELANIA_NATIVE_MANAGED_XDG_MAXIMIZED=false' \
  'MELANIA_NATIVE_MANAGED_XDG_FULLSCREEN=true' \
  'MELANIA_NATIVE_MANAGED_XDG_FULLSCREEN=false'; do
  grep -q "^$role_event$" "$shell_log" || fatal "real_xdg_role_server_event_missing:$role_event"
done
grep -q '^MELANIA_NATIVE_SHELL_PROTOCOLS_READY=true' "$shell_log" ||
  fatal 'native_xdg_layer_shell_server_not_configured'
grep -q '^MELANIA_NATIVE_SHELL_RENDERED=false' "$shell_log" ||
  fatal 'native_wayland_shell_claimed_graphical_rendering'
rm -f "$shell_log" "$shell_client_log"
report 'MELANIA_QEMU_NATIVE_PANEL_CONTROL_RESTORE_PASS=true'
report 'MELANIA_QEMU_NATIVE_XDG_ROLE_ROUNDTRIP_PASS=true'
report 'MELANIA_QEMU_NATIVE_SHELL_PROBE_PASS=true'

# Stage83F: REAL libwayland client requests all five GTK-compatible layer
# namespaces. This is a configure/ACK protocol probe, not a GTK screenshot.
gtk_role_client=/mnt/melania-native/melania-gtk-five-roles
[[ -x "$gtk_role_client" ]] || fatal 'gtk_five_role_protocol_client_missing'
gtk_server_log="$(mktemp -t melania-gtk-roles-server-XXXXXX)"
gtk_client_log="$(mktemp -t melania-gtk-roles-client-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_GTK_MULTISURFACE=YES_QEMU_VM \
MELANIA_NATIVE_SHELL_PROBE=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 22s \
  "$binary" --experimental-wayland-seat < /dev/tty1 > "$gtk_server_log" 2>&1 &
gtk_server_pid=$!
gtk_socket=''
for attempt in $(seq 1 100); do
  gtk_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$gtk_server_log" | head -n 1)"
  [[ -n "$gtk_socket" ]] && break
  kill -0 "$gtk_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$gtk_socket" ]]; then
  cat "$gtk_server_log"
  kill "$gtk_server_pid" 2>/dev/null || true
  wait "$gtk_server_pid" 2>/dev/null || true
  fatal 'gtk_five_role_protocol_socket_missing'
fi
if ! MELANIA_QEMU_GTK_FIVE_ROLES=YES_QEMU_VM WAYLAND_DISPLAY="$gtk_socket" \
  timeout --signal=TERM --kill-after=3s 12s "$gtk_role_client" \
  > "$gtk_client_log" 2>&1; then
  cat "$gtk_client_log" "$gtk_server_log"
  kill "$gtk_server_pid" 2>/dev/null || true
  wait "$gtk_server_pid" 2>/dev/null || true
  fatal 'gtk_five_role_protocol_client_failed'
fi
if ! wait "$gtk_server_pid"; then
  cat "$gtk_server_log"
  fatal 'gtk_five_role_protocol_server_failed'
fi
cat "$gtk_client_log" "$gtk_server_log"
grep -Fqx 'MELANIA_QEMU_GTK_REQUIRED_GLOBALS_PASS=true' "$gtk_client_log" ||
  fatal 'gtk4_required_wayland_globals_missing'
grep -q '^MELANIA_QEMU_GTK_FIVE_ROLE_CONFIGURES_PASS=true' "$gtk_client_log" ||
  fatal 'gtk_five_role_client_ACKs_missing'
grep -q '^MELANIA_NATIVE_LAYER_SURFACE_COUNT=5$' "$gtk_server_log" ||
  fatal 'gtk_five_role_server_count_missing'
for spec in 'melania-panel size=1280x52' 'melania-start size=510x565' 'melania-quick size=350x430' 'melania-notifications size=380x440' 'melania-calendar size=330x315'; do
  grep -Fqx "MELANIA_QEMU_GTK_ROLE_CONFIGURE_ACK=$spec" "$gtk_client_log" ||
    fatal "gtk_client_missing:$spec"
  grep -Fqx "MELANIA_NATIVE_GTK_LAYER_CONFIGURED=$spec" "$gtk_server_log" ||
    fatal "gtk_server_missing:$spec"
done
rm -f "$gtk_client_log" "$gtk_server_log"
report 'MELANIA_QEMU_GTK_FIVE_REAL_LAYER_ROLE_ACK_PASS=true'

# Stage83G: launch the actual Rust/GTK4 melania-panel executable, not a
# fabricated Wayland client. The QEMU bridge checks for its genuine Top layer
# role; GtkApplication's own startup profile proves that widgets were built.
# This is intentionally NOT a GTK frame / QMP / input / login claim.
gtk_smoke_server_log="$(mktemp -t melania-real-gtk-server-XXXXXX)"
gtk_smoke_client_log="$(mktemp -t melania-real-gtk-client-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_GTK_MULTISURFACE=YES_QEMU_VM \
MELANIA_NATIVE_SHELL_PROBE=YES_QEMU_VM \
MELANIA_NATIVE_REAL_GTK_SMOKE=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 26s \
  "$binary" --experimental-wayland-seat < /dev/tty1 \
  > "$gtk_smoke_server_log" 2>&1 &
gtk_smoke_server_pid=$!
gtk_smoke_socket=''
for attempt in $(seq 1 100); do
  gtk_smoke_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$gtk_smoke_server_log" | head -n 1)"
  [[ -n "$gtk_smoke_socket" ]] && break
  kill -0 "$gtk_smoke_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$gtk_smoke_socket" ]]; then
  cat "$gtk_smoke_server_log"
  kill "$gtk_smoke_server_pid" 2>/dev/null || true
  wait "$gtk_smoke_server_pid" 2>/dev/null || true
  fatal 'real_gtk_panel_wayland_socket_missing'
fi
mkdir -p /run/melania-native-qemu/config
LD_LIBRARY_PATH=/mnt/melania-native \
MELANIA_BUNDLE_DATA_DIR=/mnt/melania-native \
MELANIA_QEMU_VERIFY_FLUENT_LOOKUP=YES_DISPOSABLE_QEMU \
GDK_BACKEND=wayland GSK_RENDERER=cairo GSETTINGS_BACKEND=memory \
GTK_A11Y=none XDG_CONFIG_HOME=/run/melania-native-qemu/config \
WAYLAND_DISPLAY="$gtk_smoke_socket" MELANIA_PROFILE_START=1 \
  timeout --signal=TERM --kill-after=3s 20s "$gtk_panel" \
  > "$gtk_smoke_client_log" 2>&1 &
gtk_smoke_client_pid=$!
if ! wait "$gtk_smoke_server_pid"; then
  cat "$gtk_smoke_client_log" "$gtk_smoke_server_log"
  kill "$gtk_smoke_client_pid" 2>/dev/null || true
  wait "$gtk_smoke_client_pid" 2>/dev/null || true
  fatal 'real_gtk_panel_native_wayland_server_failed'
fi
kill "$gtk_smoke_client_pid" 2>/dev/null || true
wait "$gtk_smoke_client_pid" 2>/dev/null || true
cat "$gtk_smoke_client_log" "$gtk_smoke_server_log"
grep -q '^MELANIA_PANEL_BOOT_MS ' "$gtk_smoke_client_log" ||
  fatal 'real_gtk_panel_widget_build_not_confirmed'
grep -Fqx 'MELANIA_PANEL_ICON_THEME_SELECTED=Melania-Fluent-Light' "$gtk_smoke_client_log" ||
  fatal 'real_gtk_smoke_used_Adwaita_instead_of_Melania_Fluent'
grep -Fqx 'MELANIA_QEMU_GTK_FLUENT_ICON_LOOKUPS_PASS=true' "$gtk_smoke_client_log" ||
  fatal 'real_gtk_smoke_Fluent_icons_not_resolved_by_GTK4'
grep -q '^MELANIA_NATIVE_GTK_LAYER_CONFIGURED=melania-panel size=1280x52' "$gtk_smoke_server_log" ||
  fatal 'real_gtk_panel_top_layer_not_configured'
grep -q '^MELANIA_NATIVE_REAL_GTK_PANEL_LAYER_CONFIGURED=true' "$gtk_smoke_server_log" ||
  fatal 'real_gtk_panel_native_layer_not_seen'
rm -f "$gtk_smoke_client_log" "$gtk_smoke_server_log"
report 'MELANIA_QEMU_REAL_GTK_PANEL_STARTUP_PASS=true'

# Stage84: a SEPARATE opt-in run takes wl_shm pixels from the ACTUAL GTK4
# melania-panel and sends the owned copy through Smithay -> Rust DRM/KMS.
# The host independently verifies a live QMP screendump of the guest CRTC.
# No mock Wayland client, synthetic panel pixels, or production login.
# Resolve and validate the actual guest DRM receiver BEFORE launching
# background clients. Previous KMS stages declare this name much later.
continuous_kms=/mnt/melania-native/melania-continuous-kms
[[ -x "$continuous_kms" ]] || fatal 'real_gtk_KMS_Rust_receiver_missing'
gtk_kms_server_log="$(mktemp -t melania-gtk-kms-server-XXXXXX)"
gtk_kms_client_log="$(mktemp -t melania-gtk-kms-client-XXXXXX)"
gtk_kms_drm_log="$(mktemp -t melania-gtk-kms-drm-XXXXXX)"
rm -f /run/melania-native-qemu/continuous-frame-stream.sock
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_DYNAMIC_SCENE=YES_QEMU_VM \
MELANIA_NATIVE_LAYER_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_GTK_MULTISURFACE=YES_QEMU_VM \
MELANIA_NATIVE_REAL_GTK_PIXEL_KMS=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 47s \
  "$binary" --experimental-wayland-seat < /dev/tty1 \
  > "$gtk_kms_server_log" 2>&1 &
gtk_kms_server_pid=$!
gtk_kms_socket=''
for attempt in $(seq 1 100); do
  gtk_kms_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$gtk_kms_server_log" | head -n 1)"
  [[ -n "$gtk_kms_socket" && -S /run/melania-native-qemu/continuous-frame-stream.sock ]] && break
  kill -0 "$gtk_kms_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$gtk_kms_socket" || ! -S /run/melania-native-qemu/continuous-frame-stream.sock ]]; then
  cat "$gtk_kms_server_log"
  kill "$gtk_kms_server_pid" 2>/dev/null || true
  wait "$gtk_kms_server_pid" 2>/dev/null || true
  fatal 'real_gtk_KMS_server_or_stream_unavailable'
fi
# The serial watcher must receive the unique marker while the GTK frame
# is still on the virtual CRTC; after restore the pixels are no longer there.
tail -n +1 -f -s 0.05 "$gtk_kms_drm_log" > /dev/ttyS0 &
gtk_kms_tail_pid=$!
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_CONTINUOUS_FRAMES=1 \
MELANIA_QEMU_REAL_GTK_PANEL_KMS=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 40s \
  "$continuous_kms" < /dev/tty1 > "$gtk_kms_drm_log" 2>&1 &
gtk_kms_drm_pid=$!
for attempt in $(seq 1 100); do
  grep -q '^MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true' "$gtk_kms_drm_log" && break
  kill -0 "$gtk_kms_drm_pid" 2>/dev/null || break
  sleep .1
done
if ! grep -q '^MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true' "$gtk_kms_drm_log"; then
  cat "$gtk_kms_server_log" "$gtk_kms_drm_log"
  kill "$gtk_kms_server_pid" "$gtk_kms_drm_pid" 2>/dev/null || true
  fatal 'real_gtk_KMS_DRM_receiver_unavailable'
fi
LD_LIBRARY_PATH=/mnt/melania-native \
MELANIA_BUNDLE_DATA_DIR=/mnt/melania-native \
MELANIA_QEMU_VERIFY_FLUENT_LOOKUP=YES_DISPOSABLE_QEMU \
GDK_BACKEND=wayland GSK_RENDERER=cairo GSETTINGS_BACKEND=memory \
GTK_A11Y=none XDG_CONFIG_HOME=/run/melania-native-qemu/config \
WAYLAND_DISPLAY="$gtk_kms_socket" MELANIA_PROFILE_START=1 \
  timeout --signal=TERM --kill-after=3s 35s "$gtk_panel" \
  > "$gtk_kms_client_log" 2>&1 &
gtk_kms_client_pid=$!
if ! wait "$gtk_kms_drm_pid"; then
  cat "$gtk_kms_client_log" "$gtk_kms_server_log" "$gtk_kms_drm_log"
  kill "$gtk_kms_server_pid" "$gtk_kms_client_pid" 2>/dev/null || true
  fatal 'real_gtk_KMS_CRTC_did_not_present_genuine_buffer'
fi
if ! wait "$gtk_kms_server_pid"; then
  cat "$gtk_kms_client_log" "$gtk_kms_drm_log" "$gtk_kms_server_log"
  kill "$gtk_kms_client_pid" 2>/dev/null || true
  fatal 'real_gtk_KMS_Smithay_image_owner_or_ACK_failed'
fi
kill "$gtk_kms_client_pid" "$gtk_kms_tail_pid" 2>/dev/null || true
wait "$gtk_kms_client_pid" 2>/dev/null || true
wait "$gtk_kms_tail_pid" 2>/dev/null || true
cat "$gtk_kms_client_log" "$gtk_kms_drm_log" "$gtk_kms_server_log"
grep -q '^MELANIA_PANEL_BOOT_MS ' "$gtk_kms_client_log" ||
  fatal 'real_gtk_KMS_app_did_not_build_widgets'
grep -Fqx 'MELANIA_PANEL_ICON_THEME_SELECTED=Melania-Fluent-Light' "$gtk_kms_client_log" ||
  fatal 'real_gtk_KMS_used_Adwaita_instead_of_Melania_Fluent'
grep -Fqx 'MELANIA_QEMU_GTK_FLUENT_ICON_LOOKUPS_PASS=true' "$gtk_kms_client_log" ||
  fatal 'real_gtk_KMS_Fluent_icons_not_resolved_by_GTK4'
# The real GTK4 client negotiates a 52px layer configure but its existing
# theme's natural widget minimum is 60px. Verify the ACTUAL owned buffer
# rather than silently treating configured geometry as a screenshot.
grep -q '^MELANIA_NATIVE_LAYER_OWNED_SHM_QUEUED=1280x60 layer=Top' "$gtk_kms_server_log" ||
  fatal 'real_gtk_KMS_client_buffer_not_imported_or_wrong_geometry'
grep -Fqx 'MELANIA_NATIVE_REAL_GTK_PANEL_KMS_ACK=1' "$gtk_kms_server_log" ||
  fatal 'real_gtk_KMS_native_scene_ACK_missing'
grep -Fqx 'MELANIA_RUST_REAL_GTK_PANEL_FRAME_VISIBLE=1' "$gtk_kms_drm_log" ||
  fatal 'real_gtk_KMS_guest_CRTC_frame_not_visible'
grep -Fqx 'MELANIA_RUST_KMS_RESTORE_VERIFIED=true' "$gtk_kms_drm_log" ||
  fatal 'real_gtk_KMS_original_guest_CRTC_not_restored'
grep -q '^MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED=true' "$gtk_kms_drm_log" ||
  fatal 'real_gtk_panel_connector_route_not_restored'
grep -q '^MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED=1$' "$gtk_kms_drm_log" ||
  fatal 'kernel_scanout_route_not_verified_gtk_kms_drm_log_1'
rm -f "$gtk_kms_client_log" "$gtk_kms_drm_log" "$gtk_kms_server_log"
report 'MELANIA_QEMU_REAL_GTK_PANEL_KMS_SCANOUT_PASS=true'

# Stage85: NEW independent session, two KMS presentations of the SAME real
# GTK4 melania-panel surface. The panel repaints itself via its real CSS at
# 4.5 seconds after first presentation; a separate QMP monitor captures both.
# No saved screenshot, synthetic wl_shm client, or permanent theme changes.
gtk2_server_log="$(mktemp -t melania-gtk2-server-XXXXXX)"
gtk2_client_log="$(mktemp -t melania-gtk2-client-XXXXXX)"
gtk2_drm_log="$(mktemp -t melania-gtk2-drm-XXXXXX)"
rm -f /run/melania-native-qemu/continuous-frame-stream.sock
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_DYNAMIC_SCENE=YES_QEMU_VM \
MELANIA_NATIVE_LAYER_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_GTK_MULTISURFACE=YES_QEMU_VM \
MELANIA_NATIVE_REAL_GTK_PIXEL_KMS=YES_QEMU_VM \
MELANIA_NATIVE_REAL_GTK_TWO_FRAME=YES_QEMU_VM \
MELANIA_NATIVE_REAL_GTK_START_KMS=YES_QEMU_VM \
MELANIA_NATIVE_REAL_GTK_START_INPUT=YES_QEMU_VM \
MELANIA_NATIVE_GTK_START_FOCUS=YES_QEMU_VM \
MELANIA_CONTROL_SOCKET=/run/melania-native-qemu/gtk-panel-control.sock \
  timeout --signal=TERM --kill-after=3s 55s \
  "$binary" --experimental-wayland-seat < /dev/tty1 \
  > "$gtk2_server_log" 2>&1 &
gtk2_server_pid=$!
gtk2_socket=''
for attempt in $(seq 1 100); do
  gtk2_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$gtk2_server_log" | head -n 1)"
  [[ -n "$gtk2_socket" && -S /run/melania-native-qemu/continuous-frame-stream.sock ]] && break
  kill -0 "$gtk2_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$gtk2_socket" || ! -S /run/melania-native-qemu/continuous-frame-stream.sock ]]; then
  cat "$gtk2_server_log"
  kill "$gtk2_server_pid" 2>/dev/null || true
  wait "$gtk2_server_pid" 2>/dev/null || true
  fatal 'real_GTK_two_frame_Smithay_or_stream_missing'
fi
# The live serial marker is needed while each actual CRTC framebuffer exists.
tail -n +1 -f -s 0.05 "$gtk2_drm_log" > /dev/ttyS0 &
gtk2_tail_pid=$!
# Stage87: capture the REAL libinput and GTK shell path as it happens.
# Delayed final log concatenation cannot diagnose lost USB keyboard events.
tail -n +1 -f -s 0.05 "$gtk2_server_log" > /dev/ttyS0 &
gtk2_server_tail_pid=$!
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_CONTINUOUS_FRAMES=3 \
MELANIA_QEMU_REAL_GTK_PANEL_KMS=YES_QEMU_VM \
MELANIA_QEMU_GTK_TWO_FRAME_KMS=YES_QEMU_VM \
MELANIA_QEMU_GTK_START_KMS=YES_QEMU_VM \
MELANIA_QEMU_GTK_SEARCH_INPUT=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 48s \
  "$continuous_kms" < /dev/tty1 > "$gtk2_drm_log" 2>&1 &
gtk2_drm_pid=$!
for attempt in $(seq 1 100); do
  grep -q '^MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true' "$gtk2_drm_log" && break
  kill -0 "$gtk2_drm_pid" 2>/dev/null || break
  sleep .1
done
if ! grep -q '^MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true' "$gtk2_drm_log"; then
  cat "$gtk2_server_log" "$gtk2_drm_log"
  kill "$gtk2_server_pid" "$gtk2_drm_pid" 2>/dev/null || true
  fatal 'real_GTK_two_frame_DRM_receiver_unavailable'
fi
LD_LIBRARY_PATH=/mnt/melania-native \
MELANIA_BUNDLE_DATA_DIR=/mnt/melania-native \
MELANIA_QEMU_VERIFY_FLUENT_LOOKUP=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_GTK_SECOND_FRAME=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_GTK_START_FROM_SUPER=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_GTK_SEARCH_INPUT=YES_DISPOSABLE_QEMU \
MELANIA_CONTROL_SOCKET=/run/melania-native-qemu/gtk-panel-control.sock \
GDK_BACKEND=wayland GSK_RENDERER=cairo GSETTINGS_BACKEND=memory \
GTK_A11Y=none XDG_CONFIG_HOME=/run/melania-native-qemu/config \
WAYLAND_DISPLAY="$gtk2_socket" MELANIA_PROFILE_START=1 \
  timeout --signal=TERM --kill-after=3s 44s "$gtk_panel" \
  > "$gtk2_client_log" 2>&1 &
gtk2_client_pid=$!
if ! wait "$gtk2_drm_pid"; then
  cat "$gtk2_client_log" "$gtk2_server_log" "$gtk2_drm_log"
  kill "$gtk2_server_pid" "$gtk2_client_pid" 2>/dev/null || true
  fatal 'real_GTK_two_distinct_frames_not_presented_on_CRTC'
fi
if ! wait "$gtk2_server_pid"; then
  cat "$gtk2_client_log" "$gtk2_drm_log" "$gtk2_server_log"
  kill "$gtk2_client_pid" 2>/dev/null || true
  fatal 'real_GTK_two_frame_Smithay_ACK_or_owner_failed'
fi
kill "$gtk2_client_pid" "$gtk2_tail_pid" "$gtk2_server_tail_pid" 2>/dev/null || true
wait "$gtk2_client_pid" 2>/dev/null || true
wait "$gtk2_tail_pid" 2>/dev/null || true
cat "$gtk2_client_log" "$gtk2_drm_log" "$gtk2_server_log"
grep -Fqx 'MELANIA_QEMU_GTK_SECOND_REAL_FRAME_REQUESTED=true' "$gtk2_client_log" ||
  fatal 'real_GTK_second_frame_not_repainted_by_client'
grep -Fqx 'MELANIA_QEMU_GTK_FLUENT_ICON_LOOKUPS_PASS=true' "$gtk2_client_log" ||
  fatal 'real_GTK_two_frame_Fluent_lookup_missing'
grep -Fqx 'MELANIA_NATIVE_REAL_GTK_PANEL_KMS_ACK=3' "$gtk2_server_log" ||
  fatal 'real_GTK_two_frame_Smithay_owned_buffer_ACK_missing'
grep -Fqx 'MELANIA_NATIVE_REAL_GTK_TWO_FRAME_KMS_PASS=true' "$gtk2_server_log" ||
  fatal 'real_GTK_two_frame_Smithay_validation_failed'
grep -Fqx 'MELANIA_RUST_REAL_GTK_TWO_FRAME_KMS_PASS=true' "$gtk2_drm_log" ||
  fatal 'real_GTK_second_frame_CRTC_verification_failed'
grep -Fqx 'MELANIA_NATIVE_GTK_START_REAL_KEYBOARD_FOCUS=true' "$gtk2_server_log" ||
  fatal 'real_GTK_Start_wl_shm_not_keyboard_focused'
grep -Fqx 'MELANIA_QEMU_GTK_START_REAL_SEARCH_TYPED=true' "$gtk2_client_log" ||
  fatal 'real_GTK_Start_search_entry_not_typed_via_USB'
grep -Fqx 'MELANIA_NATIVE_GTK_SUPER_KEY_RELEASED=true' "$gtk2_server_log" ||
  fatal 'real_USB_Super_key_not_delivered_by_libinput'
grep -Fqx 'MELANIA_QEMU_GTK_START_SUPER_IPC_OPEN=true' "$gtk2_client_log" ||
  fatal 'real_GTK_Start_not_opened_by_private_Super_IPC'
grep -Fqx 'MELANIA_QEMU_GTK_START_OVERLAY_PRESENTED=true' "$gtk2_client_log" ||
  fatal 'real_GTK_Start_surface_not_presented_by_client'
grep -Fqx 'MELANIA_NATIVE_REAL_GTK_START_OVERLAY_KMS_PASS=true' "$gtk2_server_log" ||
  fatal 'real_GTK_Start_surface_not_owned_by_Smithay'
grep -Fqx 'MELANIA_RUST_REAL_GTK_START_OVERLAY_KMS_PASS=true' "$gtk2_drm_log" ||
  fatal 'real_GTK_Start_third_CRTC_FB_ID_not_verified'
grep -Fqx 'MELANIA_RUST_KMS_RESTORE_VERIFIED=true' "$gtk2_drm_log" ||
  fatal 'real_GTK_two_frame_CRTC_not_restored'
grep -q '^MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED=true' "$gtk2_drm_log" ||
  fatal 'real_gtk_two_frame_connector_route_not_restored'
grep -q '^MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED=3$' "$gtk2_drm_log" ||
  fatal 'kernel_scanout_route_not_verified_gtk2_drm_log_3'
rm -f "$gtk2_client_log" "$gtk2_drm_log" "$gtk2_server_log"
report 'MELANIA_QEMU_REAL_GTK_TWO_FRAME_KMS_SCANOUT_PASS=true'
report 'MELANIA_QEMU_REAL_GTK_START_OVERLAY_KMS_SCANOUT_PASS=true'

# Stage 26: The Rust native Smithay server accepts an actual Wayland xdg
# client's wl_shm XRGB8888 buffer and composes the client's pixels into
# an immutable QEMU-only framebuffer snapshot. A separate short-lived
# C/libdrm diagnostic presents those EXACT pixels and restores the CRTC.
# This is not a live Rust KMS compositor or a GTK application session.
pixel_client=/mnt/melania-native/melania-pixel-client
pixel_scanout=/mnt/melania-native/melania-pixel-scanout
rust_kms=/mnt/melania-native/melania-rust-kms
[[ -x "$rust_kms" ]] || fatal 'QEMU_Rust_two_frame_KMS_binary_missing'
[[ -x "$pixel_client" && -x "$pixel_scanout" ]] ||
  fatal 'QEMU_real_client_window_probes_missing'
pixel_log="$(mktemp -t melania-pixel-bridge-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_UPDATES=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 32s \
  "$binary" --experimental-wayland-seat < /dev/tty1 > "$pixel_log" 2>&1 &
pixel_pid=$!
pixel_socket=''
for attempt in $(seq 1 100); do
  pixel_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$pixel_log" | head -n 1)"
  [[ -n "$pixel_socket" ]] && break
  kill -0 "$pixel_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$pixel_socket" ]]; then
  cat "$pixel_log"
  kill "$pixel_pid" 2>/dev/null || true
  wait "$pixel_pid" 2>/dev/null || true
  fatal 'QEMU_real_client_socket_unavailable'
fi
if ! MELANIA_NATIVE_TWO_FRAMES=YES_QEMU_VM WAYLAND_DISPLAY="$pixel_socket" \
  timeout --signal=TERM --kill-after=3s 19s "$pixel_client"; then
  cat "$pixel_log"
  kill "$pixel_pid" 2>/dev/null || true
  wait "$pixel_pid" 2>/dev/null || true
  fatal 'QEMU_real_xdg_client_wl_shm_failed'
fi
report 'MELANIA_QEMU_NATIVE_WL_OUTPUT_PASS=true'
if ! wait "$pixel_pid"; then
  cat "$pixel_log"
  fatal 'Rust_compositor_did_not_capture_client_buffer'
fi
cat "$pixel_log"
grep -q '^MELANIA_NATIVE_WL_OUTPUT_ADVERTISED=1280x800' "$pixel_log" ||
  fatal 'native_output_not_advertised'
grep -q '^MELANIA_NATIVE_CLIENT_SHM_IMPORTED=true' "$pixel_log" ||
  fatal 'wl_shm_pixels_not_imported'
grep -q '^MELANIA_NATIVE_CLIENT_FRAME_SNAPSHOT_READY=true' "$pixel_log" ||
  fatal 'Rust_client_frame_composition_missing'
[[ -f /run/melania-native-qemu/client-frame.bgra ]] ||
  fatal 'Rust_composed_frame_missing'
[[ -f /run/melania-native-qemu/client-frame-2.bgra ]] ||
  fatal 'Rust_second_client_frame_missing'
grep -q '^MELANIA_NATIVE_CLIENT_FRAME_COUNT=2' "$pixel_log" ||
  fatal 'Rust_did_not_compose_two_distinct_client_commits'
rm -f "$pixel_log"
report 'MELANIA_QEMU_NATIVE_RUST_CLIENT_FRAME_COMPOSED=true'

# Stage28: Rust drm-rs directly scans out the TWO real client-owned buffers,
# including one page_flip, and verifies original CRTC restore. This runs
# independently from the earlier C/libdrm Stage26 regression probe.
MELANIA_QEMU_RUST_TWO_FRAME_KMS=YES_DISPOSABLE_QEMU \
  timeout --signal=TERM --kill-after=3s 27s "$rust_kms" < /dev/tty1 ||
  fatal 'QEMU_Rust_drm_two_frame_page_flip_or_restore_failed'
report 'MELANIA_QEMU_NATIVE_RUST_DIRECT_KMS_PASS=true'

MELANIA_QEMU_CLIENT_WINDOW_FRAME=YES_DISPOSABLE_QEMU \
  timeout --signal=TERM --kill-after=3s 22s "$pixel_scanout" < /dev/tty1 ||
  fatal 'QEMU_client_window_scanout_or_CRTC_restore_failed'
report 'MELANIA_QEMU_NATIVE_CLIENT_WINDOW_SCANOUT_PASS=true'
report 'MELANIA_NATIVE_LIVE_KMS_RENDERING=false'

# Stage 29: the Smithay server stays ALIVE while a separate Rust drm-rs
# process presents THREE real client commits. Each wl_shm release waits for
# the DRM process to acknowledge that frame; the third ack follows restore.
# This is a bounded integration test, not a continuous native desktop.
stream_binary=/mnt/melania-native/melania-rust-stream
[[ -x "$stream_binary" ]] || fatal 'Rust_live_stream_KMS_binary_missing'
stream_server_log="$(mktemp -t melania-live-smithay-XXXXXX)"
stream_kms_log="$(mktemp -t melania-live-kms-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 39s \
  "$binary" --experimental-wayland-seat < /dev/tty1 > "$stream_server_log" 2>&1 &
stream_server_pid=$!
live_socket=''
for attempt in $(seq 1 100); do
  live_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$stream_server_log" | head -n 1)"
  [[ -n "$live_socket" ]] && grep -q '^MELANIA_NATIVE_FRAME_STREAM_SOCKET_READY=true' "$stream_server_log" && break
  kill -0 "$stream_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$live_socket" || ! -S /run/melania-native-qemu/frame-stream.sock ]]; then
  cat "$stream_server_log"
  kill "$stream_server_pid" 2>/dev/null || true
  wait "$stream_server_pid" 2>/dev/null || true
  fatal 'Smithay_guest_live_frame_stream_unavailable'
fi

MELANIA_QEMU_RUST_LIVE_STREAM_KMS=YES_DISPOSABLE_QEMU \
  timeout --signal=TERM --kill-after=3s 38s \
  "$stream_binary" < /dev/tty1 2>&1 | tee "$stream_kms_log" &
stream_kms_pid=$!
for attempt in $(seq 1 100); do
  grep -q '^MELANIA_RUST_STREAM_KMS_CONNECTED=true' "$stream_kms_log" && break
  kill -0 "$stream_kms_pid" 2>/dev/null || break
  sleep .1
done
if ! grep -q '^MELANIA_RUST_STREAM_KMS_CONNECTED=true' "$stream_kms_log"; then
  cat "$stream_server_log"
  kill "$stream_server_pid" "$stream_kms_pid" 2>/dev/null || true
  wait "$stream_server_pid" "$stream_kms_pid" 2>/dev/null || true
  fatal 'Rust_KMS_did_not_connect_to_live_Smithay_stream'
fi
if ! MELANIA_NATIVE_THREE_FRAMES=YES_QEMU_VM \
  MELANIA_NATIVE_WAIT_PRESENT_CALLBACK=YES_QEMU_VM WAYLAND_DISPLAY="$live_socket" \
  timeout --signal=TERM --kill-after=3s 31s "$pixel_client"; then
  cat "$stream_server_log"
  kill "$stream_server_pid" "$stream_kms_pid" 2>/dev/null || true
  wait "$stream_server_pid" "$stream_kms_pid" 2>/dev/null || true
  fatal 'real_Wayland_client_three_live_commits_failed'
fi
if ! wait "$stream_kms_pid"; then
  cat "$stream_server_log"
  fatal 'Rust_live_stream_page_flips_or_CRTC_restore_failed'
fi
if ! wait "$stream_server_pid"; then
  cat "$stream_server_log"
  fatal 'Smithay_live_stream_session_failed'
fi
cat "$stream_server_log"
grep -q '^MELANIA_RUST_STREAM_THREE_CLIENT_FRAMES_PASS=true' "$stream_kms_log" ||
  fatal 'Rust_live_KMS_three_presentations_missing'
grep -q '^MELANIA_RUST_KMS_RESTORE_VERIFIED=true' "$stream_kms_log" ||
  fatal 'Rust_live_stream_original_CRTC_not_restored'
grep -q '^MELANIA_NATIVE_CLIENT_FRAME_COUNT=3' "$stream_server_log" ||
  fatal 'Smithay_did_not_deliver_three_real_client_buffers'
grep -q '^MELANIA_NATIVE_KMS_STREAM_THREE_FRAMES=true' "$stream_server_log" ||
  fatal 'Smithay_native_live_stream_not_acknowledged'
grep -q '^MELANIA_NATIVE_FRAME_CALLBACKS_PRESENTED_PASS=true' "$stream_server_log" ||
  fatal 'Smithay_did_not_deliver_three_post_KMS_frame_callbacks'
grep -q '^MELANIA_NATIVE_FRAME_CALLBACK_COUNT=3' "$stream_server_log" ||
  fatal 'Smithay_frame_callback_count_incorrect'
# Stage32: the three callbacks are also tied to scheduler tickets which
# must be retired only after the individual Rust KMS ACK arrives.
for index in 1 2 3; do
  grep -q "^MELANIA_NATIVE_SCHEDULED_FRAME_ACK=$index$" "$stream_server_log" ||
    fatal "Smithay_scheduled_frame_ticket_$index_not_retired"
done
report 'MELANIA_QEMU_NATIVE_SCHEDULED_FRAMES_PASS=true'
rm -f "$stream_server_log" "$stream_kms_log"
report 'MELANIA_QEMU_NATIVE_LIVE_WAYLAND_KMS_STREAM_PASS=true'
report 'MELANIA_QEMU_NATIVE_PRESENTED_FRAME_CALLBACKS_PASS=true'
report 'MELANIA_NATIVE_CONTINUOUS_COMPOSITOR_READY=false'

# Stage33: a separate, still disposable VM experiment uses TWO independent
# xdg_toplevel clients with genuine wl_shm pixel buffers. Exactly three
# commits are presented: A, B, A. The scene must retain window A when B
# commits, and retain window B when A changes. QMP checks BOTH regions.
rm -f /run/melania-native-qemu/frame-stream.sock
multi_server_log="$(mktemp -t melania-two-window-server-XXXXXX)"
multi_kms_log="$(mktemp -t melania-two-window-kms-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_ASYNC_KMS=YES_QEMU_VM \
MELANIA_NATIVE_DRAG_REDRAW=YES_QEMU_VM \
MELANIA_NATIVE_CLOSE_REDRAW=YES_QEMU_VM \
MELANIA_NATIVE_REOPEN_REDRAW=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 74s \
  "$binary" --experimental-wayland-seat < /dev/tty1 > "$multi_server_log" 2>&1 &
multi_server_pid=$!
# Expose the REAL compositor's markers on the QEMU serial console while
# it is running. A final cat after wait() is far too late for the tablet
# injector's second click. Follow only this guest-private temporary log.
tail -n +1 -f -s 0.2 "$multi_server_log" > /dev/ttyS0 &
multi_tail_pid=$!
multi_socket=''
for attempt in $(seq 1 100); do
  multi_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$multi_server_log" | head -n 1)"
  [[ -n "$multi_socket" ]] && grep -q '^MELANIA_NATIVE_FRAME_STREAM_SOCKET_READY=true' "$multi_server_log" && break
  kill -0 "$multi_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$multi_socket" || ! -S /run/melania-native-qemu/frame-stream.sock ]]; then
  cat "$multi_server_log"
  kill "$multi_server_pid" 2>/dev/null || true
  wait "$multi_server_pid" 2>/dev/null || true
  fatal 'two_window_live_wayland_socket_unavailable'
fi
MELANIA_QEMU_RUST_LIVE_STREAM_KMS=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_QEMU_DRAG_FRAME=YES_QEMU_VM \
MELANIA_QEMU_CLOSE_FRAME=YES_QEMU_VM \
MELANIA_QEMU_REOPEN_FRAME=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 73s \
  "$stream_binary" < /dev/tty1 2>&1 | tee "$multi_kms_log" &
multi_kms_pid=$!
for attempt in $(seq 1 100); do
  grep -q '^MELANIA_RUST_STREAM_KMS_CONNECTED=true' "$multi_kms_log" && break
  kill -0 "$multi_kms_pid" 2>/dev/null || break
  sleep .1
done
if ! grep -q '^MELANIA_RUST_STREAM_KMS_CONNECTED=true' "$multi_kms_log"; then
  kill "$multi_server_pid" "$multi_kms_pid" 2>/dev/null || true
  wait "$multi_server_pid" "$multi_kms_pid" 2>/dev/null || true
  fatal 'two_window_Rust_KMS_not_connected'
fi
if ! MELANIA_NATIVE_THREE_FRAMES=YES_QEMU_VM \
  MELANIA_NATIVE_WAIT_PRESENT_CALLBACK=YES_QEMU_VM \
  MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM \
  MELANIA_NATIVE_CLOSE_REDRAW=YES_QEMU_VM \
  MELANIA_NATIVE_REOPEN_REDRAW=YES_QEMU_VM WAYLAND_DISPLAY="$multi_socket" \
  timeout --signal=TERM --kill-after=3s 53s "$pixel_client"; then
  cat "$multi_server_log"
  kill "$multi_server_pid" "$multi_kms_pid" 2>/dev/null || true
  wait "$multi_server_pid" "$multi_kms_pid" 2>/dev/null || true
  fatal 'two_window_Wayland_client_commit_failed'
fi
wait "$multi_kms_pid" || fatal 'two_window_KMS_three_presentations_failed'
wait "$multi_server_pid" || fatal 'two_window_Smithay_failed'
kill "$multi_tail_pid" 2>/dev/null || true
wait "$multi_tail_pid" 2>/dev/null || true
cat "$multi_server_log"
grep -q '^MELANIA_NATIVE_TWO_REAL_WINDOWS_COMPOSITED=true' "$multi_server_log" ||
  fatal 'two_windows_not_retained_in_scene'
grep -q '^MELANIA_NATIVE_ASYNC_KMS_WORKER_STARTED=true' "$multi_server_log" ||
  fatal 'async_KMS_worker_not_started'
for index in 1 2 3; do
  grep -q "^MELANIA_NATIVE_ASYNC_FRAME_SUBMITTED=$index$" "$multi_server_log" ||
    fatal "async_frame_$index_not_submitted"
  grep -q "^MELANIA_NATIVE_ASYNC_FRAME_COMPLETED=$index$" "$multi_server_log" ||
    fatal "async_frame_$index_not_completed"
done
grep -q '^MELANIA_NATIVE_FRAME_CALLBACK_COUNT=3' "$multi_server_log" ||
  fatal 'two_window_frame_callbacks_missing'
grep -q '^MELANIA_NATIVE_DRAG_FRAME_SUBMITTED=4' "$multi_server_log" ||
  fatal 'real_pointer_move_did_not_submit_KMS_redraw'
grep -q '^MELANIA_NATIVE_DRAG_FRAME_ACKED=4' "$multi_server_log" ||
  fatal 'drag_redraw_did_not_receive_KMS_ACK'
grep -q '^MELANIA_NATIVE_DRAG_FRAME_VISIBLE=true' "$multi_server_log" ||
  fatal 'drag_redraw_not_retained_through_scene'
grep -q '^MELANIA_RUST_COMPOSITOR_DRAG_FRAME4_PASS=true' "$multi_kms_log" ||
  fatal 'Rust_KMS_fourth_frame_not_presented'
grep -q '^MELANIA_NATIVE_CLIENT_FIRST_WINDOW_DESTROYED=true' "$multi_server_log" ||
  fatal 'real_Wayland_first_window_not_destroyed'
grep -q '^MELANIA_NATIVE_CLOSE_DAMAGE_QUEUED=true' "$multi_server_log" ||
  fatal 'first_window_close_did_not_invalidate_scene'
grep -q '^MELANIA_NATIVE_CLOSE_FRAME_SUBMITTED=5' "$multi_server_log" ||
  fatal 'fifth_close_scene_not_submitted'
grep -q '^MELANIA_NATIVE_CLOSE_FRAME_ACKED=5' "$multi_server_log" ||
  fatal 'fifth_close_scene_not_acknowledged'
grep -q '^MELANIA_NATIVE_CLOSE_FRAME_VISIBLE=true' "$multi_server_log" ||
  fatal 'fifth_close_scene_not_verified_by_Smithay'
grep -q '^MELANIA_RUST_CLOSE_FRAME5_PASS=true' "$multi_kms_log" ||
  fatal 'Rust_KMS_fifth_close_frame_not_presented'
grep -q '^MELANIA_NATIVE_CLIENT_REOPENED_XDG_COMMITTED=true' "$multi_server_log" ||
  fatal 'real_new_client_xdg_toplevel_not_committed'
grep -q '^MELANIA_NATIVE_REOPEN_WINDOW_SLOT=1' "$multi_server_log" ||
  fatal 'new_Wayland_client_did_not_reuse_freed_slot_one'
grep -q '^MELANIA_NATIVE_REOPEN_FRAME_SUBMITTED=6' "$multi_server_log" ||
  fatal 'sixth_reopen_frame_not_scheduled'
grep -q '^MELANIA_NATIVE_REOPEN_FRAME_ACKED=6' "$multi_server_log" ||
  fatal 'sixth_reopen_frame_not_acknowledged'
grep -q '^MELANIA_NATIVE_REOPEN_FRAME_CALLBACK_DONE=true' "$multi_server_log" ||
  fatal 'real_reopened_Wayland_client_callback_not_dispatched_after_KMS'
grep -q '^MELANIA_NATIVE_REOPEN_FRAME_VISIBLE=true' "$multi_server_log" ||
  fatal 'reopened_client_not_retained_through_KMS'
grep -q '^MELANIA_RUST_REOPEN_FRAME6_PASS=true' "$multi_kms_log" ||
  fatal 'Rust_KMS_sixth_reopen_frame_not_presented'
# Unlike a source-text-only check, these records are written by the
# generation planner after the genuine DRM/KMS ACK. Expect exactly six
# strictly increasing scene generations during the two-client VM exercise.
if ! awk -F= '/^MELANIA_NATIVE_SCENE_GENERATION_ACK=/ {
    generation=$2+0
    if (count>0 && generation<=previous) bad=1
    previous=generation
    count++
} END {if (count!=6 || bad) exit 1}' "$multi_server_log"; then
  fatal 'native_scene_generation_sequence_not_acknowledged_by_KMS'
fi
report 'MELANIA_QEMU_SCENE_GENERATIONS_ACKED=6'
grep -q '^MELANIA_RUST_KMS_RESTORE_VERIFIED=true' "$multi_kms_log" ||
  fatal 'two_window_KMS_original_CRTC_not_restored'
for index in 1 2 3; do
  grep -q "^MELANIA_NATIVE_SCHEDULED_FRAME_ACK=$index$" "$multi_server_log" ||
    fatal "two_window_frame_ticket_$index_not_retired"
done
rm -f "$multi_server_log" "$multi_kms_log"
report 'MELANIA_QEMU_TWO_REAL_WINDOWS_COMPOSITED_PASS=true'
report 'MELANIA_QEMU_NONBLOCKING_KMS_WORKER_PASS=true'
report 'MELANIA_QEMU_REAL_WINDOW_CLOSE_KMS_FRAME5_PASS=true'
report 'MELANIA_QEMU_REAL_WINDOW_REOPEN_KMS_FRAME6_PASS=true'
report 'MELANIA_NATIVE_CONTINUOUS_COMPOSITOR_READY=false'

# Stage46: a SEPARATE disposable QEMU test must prove more than six
# successive *real* Wayland wl_shm commits, KMS CRTC FB_ID acknowledgements
# and frame.done/buffer.release pairs. Legacy stages 1–6 remain unchanged.
continuous_server=/mnt/melania-native/melania-native
continuous_kms=/mnt/melania-native/melania-continuous-kms
continuous_client=/mnt/melania-native/melania-continuous-client
[[ -x "$continuous_kms" && -x "$continuous_client" ]] ||
  fatal 'continuous_VM_executables_missing'
continuous_server_log="$(mktemp -t melania-continuous-server-XXXXXX)"
continuous_kms_log="$(mktemp -t melania-continuous-kms-XXXXXX)"
continuous_client_log="$(mktemp -t melania-continuous-client-XXXXXX)"
# Emit frame-visible markers WHILE the DRM framebuffer is still active.
# QMP on the host observes the real pixels before the CRTC is restored.
tail -n +1 -f -s 0.05 "$continuous_kms_log" > /dev/ttyS0 &
continuous_tail_pid=$!
rm -f /run/melania-native-qemu/continuous-frame-stream.sock
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 72s \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$continuous_server_log" 2>&1 &
continuous_server_pid=$!
continuous_socket=''
for attempt in $(seq 1 100); do
  continuous_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$continuous_server_log" | head -n 1)"
  [[ -n "$continuous_socket" && -S /run/melania-native-qemu/continuous-frame-stream.sock ]] && break
  kill -0 "$continuous_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$continuous_socket" || ! -S /run/melania-native-qemu/continuous-frame-stream.sock ]]; then
  cat "$continuous_server_log"
  fatal 'continuous_Wayland_bridge_never_listened'
fi
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_CONTINUOUS_FRAMES=12 \
  timeout --signal=TERM --kill-after=3s 65s \
  "$continuous_kms" < /dev/tty1 > "$continuous_kms_log" 2>&1 &
continuous_kms_pid=$!
for attempt in $(seq 1 100); do
  grep -q '^MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true' "$continuous_kms_log" && break
  kill -0 "$continuous_kms_pid" 2>/dev/null || break
  sleep .1
done
if ! grep -q '^MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true' "$continuous_kms_log"; then
  cat "$continuous_kms_log" "$continuous_server_log"
  kill "$continuous_server_pid" "$continuous_kms_pid" 2>/dev/null || true
  fatal 'continuous_Rust_KMS_not_connected'
fi
if ! MELANIA_QEMU_CONTINUOUS_CLIENT=YES_QEMU_VM WAYLAND_DISPLAY="$continuous_socket" \
  timeout --signal=TERM --kill-after=3s 62s \
  "$continuous_client" > "$continuous_client_log" 2>&1; then
  cat "$continuous_client_log" "$continuous_kms_log" "$continuous_server_log"
  kill "$continuous_server_pid" "$continuous_kms_pid" 2>/dev/null || true
  fatal 'continuous_real_Wayland_client_failed'
fi
if ! wait "$continuous_kms_pid"; then
  cat "$continuous_kms_log" "$continuous_server_log"
  kill "$continuous_server_pid" 2>/dev/null || true
  fatal 'continuous_real_KMS_receiver_failed'
fi
if ! wait "$continuous_server_pid"; then
  cat "$continuous_server_log"
  fatal 'continuous_Smithay_bridge_failed'
fi
kill "$continuous_tail_pid" 2>/dev/null || true
wait "$continuous_tail_pid" 2>/dev/null || true
cat "$continuous_client_log" "$continuous_kms_log" "$continuous_server_log"
for index in $(seq 1 12); do
  grep -q "^MELANIA_NATIVE_CONTINUOUS_CLIENT_ACKED=$index$" "$continuous_client_log" ||
    fatal "continuous_client_callback_$index_not_presented"
  grep -q "^MELANIA_NATIVE_CONTINUOUS_KMS_ACK=$index " "$continuous_server_log" ||
    fatal "continuous_Smithay_generation_$index_not_acknowledged"
  grep -q "^MELANIA_RUST_CONTINUOUS_FRAME_ACKED=$index " "$continuous_kms_log" ||
    fatal "continuous_real_CRTC_frame_$index_not_verified"
done
grep -q '^MELANIA_NATIVE_CONTINUOUS_WAYLAND_KMS_FRAMES_PASS=12' "$continuous_server_log" ||
  fatal 'continuous_Wayland_KMS_twelve_frames_not_completed'
grep -q '^MELANIA_NATIVE_CONTINUOUS_CLIENT_12_FRAMES_PASS=true' "$continuous_client_log" ||
  fatal 'continuous_client_did_not_finish_twelve_commits'
grep -q '^MELANIA_RUST_CONTINUOUS_FRAMES_PASS=12' "$continuous_kms_log" ||
  fatal 'continuous_Rust_CRTC_verification_incomplete'
grep -q '^MELANIA_RUST_KMS_RESTORE_VERIFIED=true' "$continuous_kms_log" ||
  fatal 'continuous_original_CRTC_not_restored'
grep -q '^MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED=true' "$continuous_kms_log" ||
  fatal 'continuous_connector_route_not_restored'
grep -q '^MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED=12$' "$continuous_kms_log" ||
  fatal 'kernel_scanout_route_not_verified_continuous_kms_log_12'
rm -f "$continuous_server_log" "$continuous_kms_log" "$continuous_client_log"
report 'MELANIA_QEMU_NATIVE_CONTINUOUS_12_REAL_FRAMES_PASS=true'
report 'MELANIA_NATIVE_CONTINUOUS_COMPOSITOR_READY=false'

# ONE-PROCESS native integration proof: REAL Smithay Wayland server and
# Rust DRM/KMS presenter now share ONE melania-native process. No external
# melania-continuous-kms executable may be launched in this scenario.
integrated_server_log="$(mktemp -t melania-integrated-server-XXXXXX)"
integrated_client_log="$(mktemp -t melania-integrated-client-XXXXXX)"
tail -n +1 -f -s 0.05 "$integrated_server_log" > /dev/ttyS0 &
integrated_tail_pid=$!
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM \
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_CONTINUOUS_FRAMES=12 \
  timeout --signal=TERM --kill-after=3s 72s \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$integrated_server_log" 2>&1 &
integrated_server_pid=$!
integrated_socket=''
for attempt in $(seq 1 100); do
  integrated_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$integrated_server_log" | head -n 1)"
  [[ -n "$integrated_socket" ]] && break
  kill -0 "$integrated_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$integrated_socket" ]] ||
   ! grep -q '^MELANIA_NATIVE_IN_PROCESS_KMS_WORKER_STARTED=true' "$integrated_server_log"; then
  cat "$integrated_server_log"
  kill "$integrated_server_pid" 2>/dev/null || true
  fatal 'integrated_Smithay_KMS_process_never_started'
fi
if ! MELANIA_QEMU_CONTINUOUS_CLIENT=YES_QEMU_VM WAYLAND_DISPLAY="$integrated_socket" \
  timeout --signal=TERM --kill-after=3s 62s \
  "$continuous_client" > "$integrated_client_log" 2>&1; then
  cat "$integrated_server_log" "$integrated_client_log"
  kill "$integrated_server_pid" 2>/dev/null || true
  fatal 'integrated_real_Wayland_client_failed'
fi
if ! wait "$integrated_server_pid"; then
  cat "$integrated_server_log" "$integrated_client_log"
  fatal 'integrated_real_Smithay_DRM_process_failed'
fi
kill "$integrated_tail_pid" 2>/dev/null || true
wait "$integrated_tail_pid" 2>/dev/null || true
for index in $(seq 1 12); do
  grep -q "^MELANIA_RUST_CONTINUOUS_FRAME_ACKED=$index " "$integrated_server_log" ||
    fatal "integrated_kernel_frame_$index_not_acknowledged"
  grep -q "^MELANIA_NATIVE_CONTINUOUS_KMS_ACK=$index " "$integrated_server_log" ||
    fatal "integrated_Wayland_frame_$index_not_retired"
  grep -q "^MELANIA_NATIVE_CONTINUOUS_CLIENT_ACKED=$index$" "$integrated_client_log" ||
    fatal "integrated_client_frame_$index_not_released"
done
grep -q '^MELANIA_RUST_KMS_FLIP_LEDGER_CONFIRMED=11$' "$integrated_server_log" ||
  fatal 'in-process presenter missing monotonic kernel flip receipts'
grep -q '^MELANIA_RUST_KMS_VBLANK_EVENT_ACK=12$' "$integrated_server_log" ||
  fatal 'in-process renderer missing real DRM vblank completion event'
grep -q '^MELANIA_RUST_KMS_TOPOLOGY_STABLE_OUTPUTS=1$' "$integrated_server_log" ||
  fatal 'integrated_DRM_topology_preflight_not_confirmed'
grep -q '^MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED=12$' "$integrated_server_log" ||
  fatal 'integrated_kernel_CRTC_connector_route_not_verified'
grep -q '^MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED=true' "$integrated_server_log" ||
  fatal 'integrated_original_connector_not_restored'
grep -q '^MELANIA_RUST_KMS_RESTORE_VERIFIED=true' "$integrated_server_log" ||
  fatal 'integrated_original_CRTC_not_restored'
grep -q '^MELANIA_RUST_CONTINUOUS_FRAMES_PASS=12' "$integrated_server_log" ||
  fatal 'integrated_receiver_never_completed'
grep -q '^MELANIA_NATIVE_IN_PROCESS_KMS_FRAMES_PASS=12' "$integrated_server_log" ||
  fatal 'integrated_Wayland_DRM_join_incomplete'
grep -q '^MELANIA_NATIVE_LIBSEAT_DRM_ACQUIRED=true' "$integrated_server_log" ||
  fatal 'integrated_DRM_card_not_acquired_through_libseat'
grep -q '^MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true' "$integrated_server_log" ||
  fatal 'integrated_libseat_DRM_card_not_closed_after_rollback'
rm -f "$integrated_server_log" "$integrated_client_log"
report 'MELANIA_QEMU_IN_PROCESS_WAYLAND_KMS_12_FRAMES_PASS=true'
report 'MELANIA_NATIVE_SESSION_READY=false'

# Sustain 24 successive real Wayland/client buffer commits and kernel KMS
# acknowledgements inside ONE compositor process. This is a longer stress
# scenario, not an unbounded production session or a hardware VT-resume proof.
# The old twelve-frame acceptance remains unchanged and is checked above.
long_server_log="$(mktemp -t melania-integrated-long-server-XXXXXX)"
long_client_log="$(mktemp -t melania-integrated-long-client-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_QEMU_FRAME_TARGET=24 \
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_CONTINUOUS_FRAMES=24 \
  timeout --signal=TERM --kill-after=3s 110s \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$long_server_log" 2>&1 &
long_server_pid=$!
long_socket=''
for attempt in $(seq 1 100); do
  long_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$long_server_log" | head -n 1)"
  [[ -n "$long_socket" ]] && break
  kill -0 "$long_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$long_socket" ]] ||
   ! grep -q '^MELANIA_NATIVE_IN_PROCESS_KMS_WORKER_STARTED=true' "$long_server_log"; then
  cat "$long_server_log"
  kill "$long_server_pid" 2>/dev/null || true
  fatal 'long_integrated_Smithay_KMS_process_never_started'
fi
if ! MELANIA_QEMU_CONTINUOUS_CLIENT=YES_QEMU_VM \
  MELANIA_QEMU_CONTINUOUS_CLIENT_FRAMES=24 WAYLAND_DISPLAY="$long_socket" \
  timeout --signal=TERM --kill-after=3s 100s \
  "$continuous_client" > "$long_client_log" 2>&1; then
  cat "$long_server_log" "$long_client_log"
  kill "$long_server_pid" 2>/dev/null || true
  fatal 'long_integrated_real_Wayland_client_failed'
fi
if ! wait "$long_server_pid"; then
  cat "$long_server_log" "$long_client_log"
  fatal 'long_integrated_real_Smithay_DRM_process_failed'
fi
for index in $(seq 1 24); do
  grep -q "^MELANIA_RUST_CONTINUOUS_FRAME_ACKED=$index " "$long_server_log" ||
    fatal "long_integrated_kernel_frame_${index}_not_acknowledged"
  grep -q "^MELANIA_NATIVE_CONTINUOUS_KMS_ACK=$index " "$long_server_log" ||
    fatal "long_integrated_Wayland_frame_${index}_not_retired"
  grep -q "^MELANIA_NATIVE_CONTINUOUS_CLIENT_ACKED=$index$" "$long_client_log" ||
    fatal "long_integrated_client_frame_${index}_not_released"
done
for marker in \
  'MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED=24' \
  'MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED=true' \
  'MELANIA_RUST_KMS_RESTORE_VERIFIED=true' \
  'MELANIA_RUST_CONTINUOUS_FRAMES_PASS=24' \
  'MELANIA_NATIVE_IN_PROCESS_KMS_FRAMES_PASS=24' \
  'MELANIA_NATIVE_LIBSEAT_DRM_ACQUIRED=true' \
  'MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true'; do
  grep -q "^${marker}$" "$long_server_log" || {
    cat "$long_server_log"
    fatal "long_integrated_missing_${marker}"
  }
done
grep -q '^MELANIA_NATIVE_CONTINUOUS_CLIENT_EXTENDED_FRAMES_PASS=24$' "$long_client_log" ||
  fatal 'long_integrated_client_extended_frames_not_completed'
rm -f "$long_server_log" "$long_client_log"
report 'MELANIA_QEMU_IN_PROCESS_WAYLAND_KMS_24_FRAMES_PASS=true'
report 'MELANIA_NATIVE_SESSION_READY=false'

# Stage109: prove a real Smithay Wayland client-owned wl_shm scene is
# imported into a GLES texture, rendered into real GPU-backed GBM DMA-BUF
# scanout targets and presented by kernel DRM KMS. This is not the
# synthetic standalone GPU clear test or CPU dumb-buffer compositor.
gpu_scene_server_log="$(mktemp -t melania-gpu-scene-server-XXXXXX)"
gpu_scene_client_log="$(mktemp -t melania-gpu-scene-client-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_GPU_WAYLAND_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_QEMU_FRAME_TARGET=24 \
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_CONTINUOUS_FRAMES=24 \
  timeout --signal=TERM --kill-after=3s 120s \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$gpu_scene_server_log" 2>&1 &
gpu_scene_server_pid=$!
gpu_scene_socket=''
for attempt in $(seq 1 100); do
  gpu_scene_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$gpu_scene_server_log" | head -n 1)"
  [[ -n "$gpu_scene_socket" ]] && break
  kill -0 "$gpu_scene_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$gpu_scene_socket" ]] ||
   ! grep -q '^MELANIA_NATIVE_IN_PROCESS_KMS_WORKER_STARTED=true' "$gpu_scene_server_log"; then
  cat "$gpu_scene_server_log"
  kill "$gpu_scene_server_pid" 2>/dev/null || true
  fatal 'GPU_Wayland_KMS_server_never_started'
fi
if ! MELANIA_QEMU_CONTINUOUS_CLIENT=YES_QEMU_VM \
  MELANIA_QEMU_CONTINUOUS_CLIENT_FRAMES=24 WAYLAND_DISPLAY="$gpu_scene_socket" \
  timeout --signal=TERM --kill-after=3s 105s \
  "$continuous_client" > "$gpu_scene_client_log" 2>&1; then
  cat "$gpu_scene_server_log" "$gpu_scene_client_log"
  kill "$gpu_scene_server_pid" 2>/dev/null || true
  fatal 'GPU_Wayland_scene_client_failed'
fi
if ! wait "$gpu_scene_server_pid"; then
  cat "$gpu_scene_server_log" "$gpu_scene_client_log"
  fatal 'GPU_Wayland_KMS_owner_failed'
fi
for index in $(seq 1 24); do
  grep -q "^MELANIA_NATIVE_GPU_WAYLAND_FRAME_VISIBLE=$index$" "$gpu_scene_server_log" ||
    fatal "GPU_real_Wayland_scene_frame_${index}_not_on_CRTC"
  grep -q "^MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=$index$" "$gpu_scene_server_log" ||
    fatal "GPU_real_Wayland_scene_frame_${index}_missing_kernel_ack"
  grep -q "^MELANIA_NATIVE_CONTINUOUS_KMS_ACK=$index " "$gpu_scene_server_log" ||
    fatal "GPU_real_Wayland_frame_${index}_not_retired"
  grep -q "^MELANIA_NATIVE_CONTINUOUS_CLIENT_ACKED=$index$" "$gpu_scene_client_log" ||
    fatal "GPU_real_Wayland_client_${index}_not_released"
done
for marker in \
  'MELANIA_NATIVE_GPU_WAYLAND_KMS_READY=true' \
  'MELANIA_NATIVE_GPU_WAYLAND_DRM_TOPOLOGY_OUTPUTS=1' \
  'MELANIA_NATIVE_GPU_WAYLAND_FLIP_LEDGER=23' \
  'MELANIA_NATIVE_GPU_WAYLAND_GBM_BUFFERS_REUSED=2' \
  'MELANIA_NATIVE_GPU_WAYLAND_24_KMS_FRAMES_PASS=true' \
  'MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true' \
  'MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true' \
  'MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true' \
  'MELANIA_NATIVE_IN_PROCESS_KMS_FRAMES_PASS=24' \
  'NATIVE_SESSION_READY=false'; do
  grep -q "^$marker$" "$gpu_scene_server_log" || {
    cat "$gpu_scene_server_log"
    fatal "GPU_Wayland_scene_missing_${marker}"
  }
done
rm -f "$gpu_scene_server_log" "$gpu_scene_client_log"
report 'MELANIA_QEMU_IN_PROCESS_WAYLAND_GBM_24_FRAMES_PASS=true'
report 'MELANIA_NATIVE_SESSION_READY=false'

# Stage110: same REAL wl_shm -> GLES -> GBM -> DRM pipeline, but it must
# remain alive without an artificial frame budget or socket idle timeout.
# The signal is delivered to the real server; libseat owns the device and
# must release it only after the GPU presenter restores the old CRTC.
persist_gpu_server_log="$(mktemp -t melania-persist-gpu-server-XXXXXX)"
persist_gpu_client_log="$(mktemp -t melania-persist-gpu-client-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_GPU_WAYLAND_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_GPU_WAYLAND=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$persist_gpu_server_log" 2>&1 &
persist_gpu_server_pid=$!
persist_gpu_socket=''
for attempt in $(seq 1 100); do
  persist_gpu_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$persist_gpu_server_log" | head -n 1)"
  [[ -n "$persist_gpu_socket" ]] && break
  kill -0 "$persist_gpu_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$persist_gpu_socket" ]] ||
   ! grep -q '^MELANIA_NATIVE_IN_PROCESS_KMS_WORKER_STARTED=true' "$persist_gpu_server_log"; then
  cat "$persist_gpu_server_log"
  kill -TERM "$persist_gpu_server_pid" 2>/dev/null || true
  fatal 'persistent_GPU_Wayland_server_not_ready'
fi
# The guest uses QEMU TCG software CPU/GPU emulation. Allow time for
# 48 *real* DRM event/GBM presentations plus the intentional 15s idle.
# Comments must stay OUTSIDE the backslash-continued environment assignment:
# otherwise Bash runs the client without its Wayland/48-frame parameters.
if ! MELANIA_QEMU_CONTINUOUS_CLIENT=YES_QEMU_VM \
  MELANIA_QEMU_CONTINUOUS_CLIENT_FRAMES=48 \
  MELANIA_QEMU_CONTINUOUS_CLIENT_IDLE_AFTER=24 MELANIA_QEMU_CONTINUOUS_CLIENT_IDENTICAL_AFTER=24 WAYLAND_DISPLAY="$persist_gpu_socket" \
  timeout --signal=TERM --kill-after=3s 240s \
  "$continuous_client" > "$persist_gpu_client_log" 2>&1; then
  cat "$persist_gpu_server_log" "$persist_gpu_client_log"
  kill -TERM "$persist_gpu_server_pid" 2>/dev/null || true
  fatal 'persistent_GPU_Wayland_client_failed'
fi
# A second idle hold after resuming and presenting 48 frames verifies
# both mid-session recovery and final quiet shutdown. In addition, the
# actual Wayland client paused for 15 seconds between frames 24 and 25.
sleep 15
if ! kill -0 "$persist_gpu_server_pid" 2>/dev/null; then
  cat "$persist_gpu_server_log" "$persist_gpu_client_log"
  fatal 'persistent_GPU_Wayland_exited_during_idle'
fi
kill -TERM "$persist_gpu_server_pid"
if ! wait "$persist_gpu_server_pid"; then
  cat "$persist_gpu_server_log" "$persist_gpu_client_log"
  fatal 'persistent_GPU_Wayland_signal_rollback_failed'
fi
for index in $(seq 1 48); do
  grep -q "^MELANIA_NATIVE_GPU_WAYLAND_FRAME_VISIBLE=$index$" "$persist_gpu_server_log" || {
    cat "$persist_gpu_server_log" "$persist_gpu_client_log"
    fatal "persistent_GPU_Wayland_CRTC_frame_${index}_missing"
  }
  grep -q "^MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=$index$" "$persist_gpu_server_log" || {
    cat "$persist_gpu_server_log" "$persist_gpu_client_log"
    fatal "persistent_GPU_Wayland_kernel_ack_${index}_missing"
  }
  grep -q "^MELANIA_NATIVE_CONTINUOUS_CLIENT_ACKED=$index$" "$persist_gpu_client_log" || {
    cat "$persist_gpu_server_log" "$persist_gpu_client_log"
    fatal "persistent_GPU_Wayland_client_ack_${index}_missing"
  }
done
for marker in \
  'MELANIA_NATIVE_GPU_WAYLAND_PERSISTENT_ROLLBACK_PASS=true' \
  'MELANIA_NATIVE_GPU_WAYLAND_GBM_BUFFERS_REUSED=2' \
  'MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true' \
  'MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true' \
  'MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true' \
  'MELANIA_NATIVE_PERSISTENT_KMS_SHUTDOWN_OK=true' \
  'NATIVE_SESSION_READY=false'; do
  grep -q "^$marker$" "$persist_gpu_server_log" || {
    cat "$persist_gpu_server_log"
    fatal "persistent_GPU_Wayland_missing_${marker}"
  }
done
grep -q '^MELANIA_NATIVE_GPU_WAYLAND_CLIENT_IDLE_AFTER_24=true$' "$persist_gpu_client_log" ||
  fatal 'GPU_Wayland_client_never_entered_idle'
grep -q '^MELANIA_NATIVE_GPU_WAYLAND_CLIENT_RESUMED_AFTER_IDLE=true$' "$persist_gpu_client_log" ||
  fatal 'GPU_Wayland_client_did_not_resume_after_idle'
grep -q '^MELANIA_NATIVE_GPU_WAYLAND_IDENTICAL_CLIENT_FRAME25_ACKED=true$' "$persist_gpu_client_log" || fatal 'GPU_Wayland_same_frame_failed'
grep -q '^MELANIA_NATIVE_GPU_WAYLAND_FLIP_LEDGER=47$' "$persist_gpu_server_log" ||
  fatal 'GPU_Wayland_kernel_flip_47_not_accepted'
grep -q '^MELANIA_NATIVE_CONTINUOUS_CLIENT_EXTENDED_FRAMES_PASS=48$' "$persist_gpu_client_log" ||
  fatal 'GPU_Wayland_client_did_not_complete_48_frames'
# The client commits exactly 48 frames. Destroying its wl_surface can
# legitimately damage the retained scene and cause an additional, genuinely
# kernel-presented cleanup frame before SIGTERM. The renderer is unbounded by
# design: require at least the 48 client frames, then audit *every* additional
# presentation instead of incorrectly demanding that the server stop at 48.
gpu_verified_frames="$(sed -n 's/^MELANIA_NATIVE_GPU_WAYLAND_PERSISTENT_STOPPED_FRAMES=//p' "$persist_gpu_server_log" | tail -n 1)"
if [[ ! "$gpu_verified_frames" =~ ^[0-9]+$ ]] ||
   ((gpu_verified_frames < 48 || gpu_verified_frames > 64)); then
  cat "$persist_gpu_server_log" "$persist_gpu_client_log"
  fatal "persistent_GPU_Wayland_invalid_terminal_frame_count_${gpu_verified_frames:-missing}"
fi
for index in $(seq 1 "$gpu_verified_frames"); do
  # Exactly one kernel-visible scanout and one acknowledged transfer per
  # index; duplicate or missing receipts must never be counted as success.
  if [[ "$(grep -cx "MELANIA_NATIVE_GPU_WAYLAND_FRAME_VISIBLE=$index" "$persist_gpu_server_log")" != 1 ]] ||
     [[ "$(grep -cx "MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=$index" "$persist_gpu_server_log")" != 1 ]]; then
    cat "$persist_gpu_server_log" "$persist_gpu_client_log"
    fatal "persistent_GPU_Wayland_unverified_terminal_frame_$index"
  fi
done
grep -qx "MELANIA_NATIVE_GPU_WAYLAND_FLIP_LEDGER=$((gpu_verified_frames - 1))" "$persist_gpu_server_log" || {
  cat "$persist_gpu_server_log" "$persist_gpu_client_log"
  fatal 'persistent_GPU_Wayland_final_kernel_flip_ledger_incomplete'
}
report "MELANIA_QEMU_PERSISTENT_GPU_VERIFIED_FRAMES=$gpu_verified_frames"
rm -f "$persist_gpu_server_log" "$persist_gpu_client_log"
report 'MELANIA_QEMU_IN_PROCESS_PERSISTENT_GPU_WAYLAND_PASS=true'
report 'MELANIA_NATIVE_SESSION_READY=false'

# Stage114: genuine Wayland pointer serial -> client-requested TopLeft resize
# -> new owned wl_shm pixels -> verified GPU/GBM/DRM scanout.
auth_client=/mnt/melania-native/melania-auth-resize-client
[[ -x "$auth_client" ]] || fatal 'authenticated_resize_real_client_missing'
auth_server_log="$(mktemp -t melania-auth-resize-server-XXXXXX)"
auth_client_log="$(mktemp -t melania-auth-resize-client-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_GPU_WAYLAND_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_GPU_WAYLAND=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_DYNAMIC_SCENE=YES_QEMU_VM \
MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM \
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$auth_server_log" 2>&1 &
auth_server_pid=$!
auth_socket=''
for attempt in $(seq 1 100); do
  auth_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$auth_server_log" | head -n 1)"
  [[ -n "$auth_socket" ]] && break
  kill -0 "$auth_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$auth_socket" ]] ||
   ! grep -q '^MELANIA_NATIVE_IN_PROCESS_KMS_WORKER_STARTED=true' "$auth_server_log"; then
  cat "$auth_server_log"
  kill -TERM "$auth_server_pid" 2>/dev/null || true
  fatal 'authenticated_resize_GPU_Wayland_server_not_ready'
fi
tail -n +1 -f -s 0.05 "$auth_server_log" > /dev/ttyS0 &
auth_server_tail_pid=$!
tail -n +1 -f -s 0.05 "$auth_client_log" > /dev/ttyS0 &
auth_client_tail_pid=$!
if ! MELANIA_QEMU_AUTH_RESIZE_CLIENT=YES_QEMU_VM WAYLAND_DISPLAY="$auth_socket" \
   timeout --signal=TERM --kill-after=3s 145s \
   "$auth_client" > "$auth_client_log" 2>&1; then
  cat "$auth_client_log" "$auth_server_log"
  kill -TERM "$auth_server_pid" 2>/dev/null || true
  fatal 'authenticated_resize_real_client_failed'
fi
kill -TERM "$auth_server_pid"
if ! wait "$auth_server_pid"; then
  cat "$auth_server_log" "$auth_client_log"
  fatal 'authenticated_resize_GPU_KMS_restore_failed'
fi
kill "$auth_server_tail_pid" "$auth_client_tail_pid" 2>/dev/null || true
wait "$auth_server_tail_pid" "$auth_client_tail_pid" 2>/dev/null || true
cat "$auth_client_log" "$auth_server_log"
for index in 1 2; do
  grep -q "^MELANIA_NATIVE_AUTH_RESIZE_FRAME$index""_ACK=true$" "$auth_client_log" ||
    fatal "authenticated_resize_client_frame$index""_KMS_callback_missing"
  grep -q "^MELANIA_NATIVE_GPU_WAYLAND_FRAME_VISIBLE=$index$" "$auth_server_log" ||
    fatal "authenticated_resize_GPU_CRTC_frame$index""_missing"
  grep -q "^MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=$index$" "$auth_server_log" ||
    fatal "authenticated_resize_kernel_frame$index""_missing"
done
for marker in \
  'MELANIA_NATIVE_XDG_CLIENT_RESIZE_GRANTED=1 edge=TopLeft' \
  'MELANIA_NATIVE_AUTH_RESIZE_REQUEST_SENT=true' \
  'MELANIA_NATIVE_AUTH_RESIZE_FINAL_CONFIGURE_ACK=true' \
  'MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true' \
  'MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true' \
  'MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true' \
  'NATIVE_SESSION_READY=false'; do
  grep -qx "$marker" "$auth_server_log" "$auth_client_log" ||
    fatal "authenticated_resize_missing_$marker"
done
grep -Eq '^MELANIA_NATIVE_MANAGED_XDG_CLIENT_BUFFER_ACCEPTED=[1-9][0-9]*x[1-9][0-9]*$' "$auth_server_log" ||
  fatal 'authenticated_resize_client_pixels_not_ingested'
rm -f "$auth_server_log" "$auth_client_log"
report 'MELANIA_QEMU_AUTHENTICATED_XDG_RESIZE_REAL_KMS_PASS=true'
report 'MELANIA_NATIVE_SESSION_READY=false'

# Stage120: genuine QMP USB Super+arrows -> XDG ACK -> client wl_shm
# -> real GBM/DRM scanout, screenshots and confirmed original CRTC restore.
snap_client=/mnt/melania-native/melania-super-snap-client
[[ -x "$snap_client" ]] || fatal 'super_snap_client_missing'
snap_server_log="$(mktemp -t melania-snap-server-XXXXXX)"
snap_client_log="$(mktemp -t melania-snap-client-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_GPU_WAYLAND_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_GPU_WAYLAND=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_DYNAMIC_SCENE=YES_QEMU_VM \
MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM \
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$snap_server_log" 2>&1 &
snap_server_pid=$!
snap_socket=''
for attempt in $(seq 1 100); do
  snap_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$snap_server_log" | head -n 1)"
  [[ -n "$snap_socket" ]] && break
  kill -0 "$snap_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$snap_socket" ]] ||
   ! grep -q '^MELANIA_NATIVE_IN_PROCESS_KMS_WORKER_STARTED=true' "$snap_server_log"; then
  cat "$snap_server_log"
  kill -TERM "$snap_server_pid" 2>/dev/null || true
  fatal 'super_snap_GPU_wayland_not_ready'
fi
tail -n +1 -f -s 0.05 "$snap_server_log" > /dev/ttyS0 &
snap_server_tail_pid=$!
tail -n +1 -f -s 0.05 "$snap_client_log" > /dev/ttyS0 &
snap_client_tail_pid=$!
report 'MELANIA_QEMU_SUPER_SNAP_STAGE_STARTED=true'
if ! MELANIA_QEMU_SUPER_SNAP_CLIENT=YES_QEMU_VM WAYLAND_DISPLAY="$snap_socket" \
   timeout --signal=TERM --kill-after=3s 135s \
   "$snap_client" > "$snap_client_log" 2>&1; then
  cat "$snap_client_log" "$snap_server_log"
  kill -TERM "$snap_server_pid" 2>/dev/null || true
  fatal 'real_super_snap_client_failed'
fi
for frame in 1 2 3 4 5; do
  grep -q "^MELANIA_NATIVE_SNAP_CLIENT_FRAME$frame""_ACK=true$" "$snap_client_log" ||
    fatal "snap_client_frame_$frame""_missing"
  grep -q "^MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=$frame$" "$snap_server_log" ||
    fatal "snap_GPU_CRTC_$frame""_missing"
done
for receipt in \
  'MELANIA_NATIVE_SNAP_CLIENT_XDG_HALF_ACK=true' \
  'MELANIA_NATIVE_SNAP_CLIENT_XDG_RESTORE_ACK=true' \
  'MELANIA_NATIVE_SNAP_CLIENT_XDG_RIGHT_THIRD_ACK=true' \
  'MELANIA_NATIVE_SNAP_CLIENT_XDG_THIRD_RESTORE_ACK=true' \
  'MELANIA_NATIVE_SNAP_CLIENT_HOST_RELEASE_ACK=true' \
  'MELANIA_NATIVE_SNAP_CLIENT_MINIMIZE_KEYBOARD_LEAVE=true' \
  'MELANIA_NATIVE_SNAP_CLIENT_ROLE_DESTROY_AFTER_MINIMIZE=true' \
  'MELANIA_NATIVE_SNAP_CLIENT_ALL_FIVE_REAL_FRAMES_PASS=true'; do
  grep -qx "$receipt" "$snap_client_log" || fatal "snap_client_missing_$receipt"
done
grep -q '^MELANIA_NATIVE_SUPER_SNAP_CONFIGURED=Some(LeftHalf) 640x800 at 0,0$' "$snap_server_log" ||
  fatal 'Super_Left_missing_real_XDG_configure'
grep -q '^MELANIA_NATIVE_SUPER_SNAP_CONFIGURED=None 360x224 at 160,120$' "$snap_server_log" ||
  fatal 'Super_Down_missing_real_XDG_restore'
grep -q '^MELANIA_NATIVE_SUPER_SNAP_CONFIGURED=Some(RightThird) 427x800 at 853,0$' "$snap_server_log" ||
  fatal 'Super_Shift_Right_missing_real_XDG_third'
[[ $(grep -c '^MELANIA_NATIVE_SUPER_SNAP_CONFIGURED=None 360x224 at 160,120$' "$snap_server_log") -ge 2 ]] ||
  fatal 'Super_Shift_Down_missing_real_XDG_restore'
grep -qx 'MELANIA_NATIVE_MANAGED_XDG_MINIMIZED=true' "$snap_server_log" ||
  fatal 'Super_Down_normal_did_not_minimize_live_XDG_role'
grep -qx 'MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=6' "$snap_server_log" ||
  fatal 'Super_Down_minimize_did_not_clear_real_KMS_frame6'
kill -TERM "$snap_server_pid"
if ! wait "$snap_server_pid"; then
  cat "$snap_server_log" "$snap_client_log"
  fatal 'snap_GPU_rollback_failed'
fi
kill "$snap_server_tail_pid" "$snap_client_tail_pid" 2>/dev/null || true
wait "$snap_server_tail_pid" "$snap_client_tail_pid" 2>/dev/null || true
for receipt in \
  'MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true' \
  'MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true' \
  'MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true' \
  'NATIVE_SESSION_READY=false'; do
  grep -qx "$receipt" "$snap_server_log" || fatal "snap_rollback_missing_$receipt"
done
rm -f "$snap_server_log" "$snap_client_log"
report 'MELANIA_QEMU_SUPER_SNAP_USB_XDG_KMS5_PASS=true'
report 'MELANIA_NATIVE_SESSION_READY=false'

# Stage121: actual QMP tablet left-button -> XDG move serial -> output edge
# -> configure/ACK -> client-owned shm -> GPU/GBM/DRM CRTC pixels.
# Only the disposable VM can exercise this, never a production login.
edge_client=/mnt/melania-native/melania-edge-drop-client
[[ -x "$edge_client" ]] || fatal 'edge_drop_real_client_missing'
edge_server_log="$(mktemp -t melania-edge-server-XXXXXX)"
edge_client_log="$(mktemp -t melania-edge-client-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_GPU_WAYLAND_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_GPU_WAYLAND=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_DYNAMIC_SCENE=YES_QEMU_VM \
MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM \
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$edge_server_log" 2>&1 &
edge_server_pid=$!
edge_socket=''
for attempt in $(seq 1 100); do
  edge_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$edge_server_log" | head -n 1)"
  [[ -n "$edge_socket" ]] && break
  kill -0 "$edge_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$edge_socket" ]] ||
   ! grep -q '^MELANIA_NATIVE_IN_PROCESS_KMS_WORKER_STARTED=true' "$edge_server_log"; then
  cat "$edge_server_log"
  kill -TERM "$edge_server_pid" 2>/dev/null || true
  fatal 'real_edge_drop_wayland_GPU_server_missing'
fi
tail -n +1 -f -s 0.05 "$edge_server_log" > /dev/ttyS0 &
edge_tail_server_pid=$!
tail -n +1 -f -s 0.05 "$edge_client_log" > /dev/ttyS0 &
edge_tail_client_pid=$!
report 'MELANIA_QEMU_EDGE_DROP_STAGE_STARTED=true'
if ! MELANIA_QEMU_EDGE_DROP_CLIENT=YES_QEMU_VM WAYLAND_DISPLAY="$edge_socket" \
   timeout --signal=TERM --kill-after=3s 135s \
   "$edge_client" > "$edge_client_log" 2>&1; then
  cat "$edge_client_log" "$edge_server_log"
  kill -TERM "$edge_server_pid" 2>/dev/null || true
  fatal 'real_edge_drop_client_failed'
fi
for receipt in \
  'MELANIA_NATIVE_EDGE_DROP_CLIENT_MOVE_REQUESTED=true' \
  'MELANIA_NATIVE_EDGE_DROP_CLIENT_XDG_HALF_ACK=true' \
  'MELANIA_NATIVE_EDGE_DROP_CLIENT_FRAME1_ACK=true' \
  'MELANIA_NATIVE_EDGE_DROP_CLIENT_FRAME2_ACK=true' \
  'MELANIA_NATIVE_EDGE_DROP_CLIENT_HOST_RELEASE_ACK=true' \
  'MELANIA_NATIVE_EDGE_DROP_CLIENT_TWO_REAL_FRAMES_PASS=true'; do
  grep -qx "$receipt" "$edge_client_log" ||
    fatal "edge_drop_missing_real_client_$receipt"
done
grep -q '^MELANIA_NATIVE_XDG_CLIENT_MOVE_GRANTED=1$' "$edge_server_log" ||
  fatal 'edge_drop_move_was_not_authenticated'
grep -q '^MELANIA_NATIVE_EDGE_DROP_REQUEST=1 Snap(LeftHalf)$' "$edge_server_log" ||
  fatal 'edge_drop_compositor_did_not_snap_to_real_left_edge'
grep -q '^MELANIA_NATIVE_MANAGED_XDG_CLIENT_BUFFER_ACCEPTED=640x800$' "$edge_server_log" ||
  fatal 'edge_drop_client_owned_shm_not_accepted'
grep -q '^MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=1$' "$edge_server_log" ||
  fatal 'edge_drop_initial_kernel_CRTC_missing'
grep -Eq '^MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=([2-9]|[1-9][0-9]+)$' "$edge_server_log" ||
  fatal 'edge_drop_second_kernel_CRTC_missing'
kill -TERM "$edge_server_pid"
if ! wait "$edge_server_pid"; then
  cat "$edge_server_log" "$edge_client_log"
  fatal 'edge_drop_KMS_connector_restore_failed'
fi
kill "$edge_tail_server_pid" "$edge_tail_client_pid" 2>/dev/null || true
wait "$edge_tail_server_pid" "$edge_tail_client_pid" 2>/dev/null || true
for receipt in \
  'MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true' \
  'MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true' \
  'MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true' \
  'NATIVE_SESSION_READY=false'; do
  grep -qx "$receipt" "$edge_server_log" ||
    fatal "edge_drop_kernel_rollback_missing_$receipt"
done
rm -f "$edge_server_log" "$edge_client_log"
report 'MELANIA_QEMU_EDGE_DROP_XDG_REAL_CRTC_PASS=true'
report 'MELANIA_NATIVE_SESSION_READY=false'

# Stage116: real USB Alt+Tab cycles two genuine Wayland clients. Unlike
# model-only tests, the client must receive wl_keyboard.Enter(B), then (A).
# The compositor must raise A and present the *owned* wl_shm scene at CRTC13.
# Keep KMS persistent solely within the disposable QEMU guest; libseat VT
# and original atomic output state must be restored on SIGTERM.
alt_client=/mnt/melania-native/melania-two-continuous-client
alt_server_log="$(mktemp -t melania-native-alt-tab-server-XXXXXX)"
alt_client_log="$(mktemp -t melania-native-alt-tab-client-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_GPU_WAYLAND_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_GPU_WAYLAND=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_DYNAMIC_SCENE=YES_QEMU_VM \
MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM \
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$alt_server_log" 2>&1 &
alt_server_pid=$!
alt_socket=''
for attempt in $(seq 1 100); do
  alt_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$alt_server_log" | head -n 1)"
  [[ -n "$alt_socket" ]] && break
  kill -0 "$alt_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$alt_socket" ]] ||
   ! grep -q '^MELANIA_NATIVE_IN_PROCESS_KMS_WORKER_STARTED=true' "$alt_server_log"; then
  cat "$alt_server_log"
  kill -TERM "$alt_server_pid" 2>/dev/null || true
  fatal 'real_USB_Alt_Tab_managed_GPU_Wayland_server_missing'
fi
tail -n +1 -f -s 0.05 "$alt_server_log" > /dev/ttyS0 &
alt_server_tail_pid=$!
tail -n +1 -f -s 0.05 "$alt_client_log" > /dev/ttyS0 &
alt_client_tail_pid=$!
if ! MELANIA_QEMU_CONTINUOUS_TWO_CLIENTS=YES_QEMU_VM \
      MELANIA_QEMU_ALT_TAB_CLIENT=YES_QEMU_VM WAYLAND_DISPLAY="$alt_socket" \
      timeout --signal=TERM --kill-after=3s 130s \
      "$alt_client" > "$alt_client_log" 2>&1; then
  cat "$alt_client_log" "$alt_server_log"
  kill -TERM "$alt_server_pid" 2>/dev/null || true
  fatal 'real_Alt_Tab_client_keyboard_focus_failed'
fi
# CRTC13 must already have reached kernel confirmation while both clients
# remained mapped for the host's independent QMP physical screen capture.
grep -q '^MELANIA_NATIVE_ALT_TAB_TWO_REAL_KEYBOARD_ENTERS_PASS=true' "$alt_client_log" ||
  fatal 'real_Wayland_keyboard_enter_B_then_A_not_observed'
grep -q '^MELANIA_NATIVE_ALT_F4_REAL_XDG_CLOSE_PASS=true' "$alt_client_log" ||
  fatal 'real_Alt_F4_XDG_close_not_received'
grep -q '^MELANIA_NATIVE_KEYBOARD_ALT_F4_CLOSE_REQUESTED=1$' "$alt_server_log" ||
  fatal 'real_Alt_F4_not_routed_to_focused_XDG_window'
grep -q '^MELANIA_NATIVE_KEYBOARD_ALT_TAB_FOCUS=2$' "$alt_server_log" ||
  fatal 'USB_Alt_Tab_B_not_focused'
grep -q '^MELANIA_NATIVE_KEYBOARD_ALT_TAB_FOCUS=1$' "$alt_server_log" ||
  fatal 'USB_Alt_Tab_A_not_focused'
grep -q '^MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=13$' "$alt_server_log" ||
  fatal 'real_Alt_Tab_gpu_CRTC13_not_confirmed'
kill -TERM "$alt_server_pid"
if ! wait "$alt_server_pid"; then
  cat "$alt_server_log" "$alt_client_log"
  fatal 'Alt_Tab_GPU_KMS_original_CRTC_restore_failed'
fi
kill "$alt_server_tail_pid" "$alt_client_tail_pid" 2>/dev/null || true
wait "$alt_server_tail_pid" "$alt_client_tail_pid" 2>/dev/null || true
for marker in \
  'MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true' \
  'MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true' \
  'MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true' \
  'NATIVE_SESSION_READY=false'; do
  grep -qx "$marker" "$alt_server_log" ||
    fatal "Alt_Tab_virtual_GPU_restore_missing_$marker"
done
rm -f "$alt_server_log" "$alt_client_log"
report 'MELANIA_QEMU_REAL_USB_ALT_TAB_WAYLAND_KMS_PASS=true'
report 'MELANIA_NATIVE_SESSION_READY=false'

# Stage129: genuine QEMU USB Super+S/Shift+S/PageUp/PageDown reaches
# native libinput -> Smithay XDG tabs -> owned wl_shm scene -> KMS flip.
# A separate host QMP watcher checks the ACTUAL virtual CRTC pixels after
# the two client windows were really mapped and each frame was acknowledged.
# Do not run this scenario on the normal login session or use fake Wayland keys.
tab_client=/mnt/melania-native/melania-two-continuous-client
tab_server_log="$(mktemp -t melania-native-tabs-server-XXXXXX)"
tab_client_log="$(mktemp -t melania-native-tabs-client-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_GPU_WAYLAND_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_GPU_WAYLAND=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_DYNAMIC_SCENE=YES_QEMU_VM \
MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM \
MELANIA_NATIVE_TAB_STACKS=YES_QEMU_VM \
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$tab_server_log" 2>&1 &
tab_server_pid=$!
tab_socket=''
for attempt in $(seq 1 100); do
  tab_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$tab_server_log" | head -n 1)"
  [[ -n "$tab_socket" ]] && break
  kill -0 "$tab_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$tab_socket" ]] ||
   ! grep -q '^MELANIA_NATIVE_IN_PROCESS_KMS_WORKER_STARTED=true' "$tab_server_log"; then
  cat "$tab_server_log"
  kill -TERM "$tab_server_pid" 2>/dev/null || true
  fatal 'real_tab_stack_Wayland_KMS_guest_start_failed'
fi
tail -n +1 -f -s 0.05 "$tab_server_log" > /dev/ttyS0 &
tab_tail_server_pid=$!
tail -n +1 -f -s 0.05 "$tab_client_log" > /dev/ttyS0 &
tab_tail_client_pid=$!
report 'MELANIA_QEMU_REAL_TAB_STACK_STAGE_STARTED=true'
if ! MELANIA_QEMU_CONTINUOUS_TWO_CLIENTS=YES_QEMU_VM \
      MELANIA_QEMU_TAB_CLIENT=YES_QEMU_VM WAYLAND_DISPLAY="$tab_socket" \
      timeout --signal=TERM --kill-after=3s 150s \
      "$tab_client" > "$tab_client_log" 2>&1; then
  cat "$tab_client_log" "$tab_server_log"
  kill -TERM "$tab_server_pid" 2>/dev/null || true
  fatal 'real_Wayland_USB_tab_client_failed'
fi
grep -q '^MELANIA_QEMU_TAB_REAL_USB_RELEASE=F8$' "$tab_client_log" ||
  fatal 'tab_stack_real_keyboard_release_missing'
grep -q '^MELANIA_QEMU_TAB_TWO_WL_KEYBOARD_ENTERS_PASS=true$' "$tab_client_log" ||
  fatal 'tab_stack_focus_B_then_A_not_received_by_real_clients'
for receipt in \
  'MELANIA_NATIVE_TAB_STACKING=true id=1' \
  'MELANIA_NATIVE_TAB_ATTACHED=1:2' \
  'MELANIA_NATIVE_TAB_ACTIVE=2' \
  'MELANIA_NATIVE_TAB_ACTIVE=1' \
  'MELANIA_NATIVE_TAB_STACKING=false id=1' \
  'MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=13' \
  'MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=14' \
  'MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=15' \
  'MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=16'; do
  grep -qx "$receipt" "$tab_server_log" ||
    fatal "tab_stack_actual_XDG_KMS_missing_$receipt"
done
kill -TERM "$tab_server_pid"
if ! wait "$tab_server_pid"; then
  cat "$tab_server_log" "$tab_client_log"
  fatal 'tab_stack_original_KMS_CRTC_restore_failed'
fi
kill "$tab_tail_server_pid" "$tab_tail_client_pid" 2>/dev/null || true
wait "$tab_tail_server_pid" "$tab_tail_client_pid" 2>/dev/null || true
for receipt in \
  'MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true' \
  'MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true' \
  'MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true' \
  'NATIVE_SESSION_READY=false'; do
  grep -qx "$receipt" "$tab_server_log" ||
    fatal "tab_stack_kernel_resource_rollback_missing_$receipt"
done
rm -f "$tab_server_log" "$tab_client_log"
report 'MELANIA_QEMU_NATIVE_REAL_TAB_STACK_USB_KMS_PASS=true'
report 'MELANIA_NATIVE_SESSION_READY=false'

# Stage130: real physical QMP USB tablet motion over two distinct retained
# XDG toplevels, no click and no alt-tab. Verify wl_keyboard Enter B then A,
# visible client-owned pixels on the already-ACKed KMS CRTC12, and VT/DRM
# rollback. Sloppy focus remains guest-only and disabled by default.
hover_server_log="$(mktemp -t melania-native-hover-server-XXXXXX)"
hover_client_log="$(mktemp -t melania-native-hover-client-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_GPU_WAYLAND_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_GPU_WAYLAND=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_DYNAMIC_SCENE=YES_QEMU_VM \
MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM \
MELANIA_NATIVE_FOCUS_FOLLOWS_POINTER=YES_QEMU_VM \
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$hover_server_log" 2>&1 &
hover_server_pid=$!
hover_socket=''
for attempt in $(seq 1 100); do
  hover_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$hover_server_log" | head -n 1)"
  [[ -n "$hover_socket" ]] && break
  kill -0 "$hover_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$hover_socket" ]] ||
   ! grep -q '^MELANIA_NATIVE_IN_PROCESS_KMS_WORKER_STARTED=true' "$hover_server_log"; then
  cat "$hover_server_log"
  kill -TERM "$hover_server_pid" 2>/dev/null || true
  fatal 'real_USB_hover_GPU_Wayland_server_not_started'
fi
tail -n +1 -f -s 0.05 "$hover_server_log" > /dev/ttyS0 &
hover_server_tail_pid=$!
tail -n +1 -f -s 0.05 "$hover_client_log" > /dev/ttyS0 &
hover_client_tail_pid=$!
if ! MELANIA_QEMU_CONTINUOUS_TWO_CLIENTS=YES_QEMU_VM \
      MELANIA_QEMU_HOVER_FOCUS_CLIENT=YES_QEMU_VM \
      WAYLAND_DISPLAY="$hover_socket" \
      timeout --signal=TERM --kill-after=3s 130s \
      "$alt_client" > "$hover_client_log" 2>&1; then
  cat "$hover_client_log" "$hover_server_log"
  kill -TERM "$hover_server_pid" 2>/dev/null || true
  fatal 'real_USB_hover_two_client_keyboard_focus_failed'
fi
grep -qx 'MELANIA_NATIVE_HOVER_FOCUS_TWO_REAL_KEYBOARD_ENTERS_PASS=true' "$hover_client_log" ||
  fatal 'real_hover_Wayland_Enter_B_then_A_missing'
grep -qx 'MELANIA_NATIVE_FOCUS_FOLLOWS_POINTER=2' "$hover_server_log" ||
  fatal 'USB_hover_B_not_focused_on_topmost_XDG'
grep -qx 'MELANIA_NATIVE_FOCUS_FOLLOWS_POINTER=1' "$hover_server_log" ||
  fatal 'USB_hover_A_not_focused_on_topmost_XDG'
grep -qx 'MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=12' "$hover_server_log" ||
  fatal 'real_hover_client_CRTC12_not_confirmed'
kill -TERM "$hover_server_pid"
if ! wait "$hover_server_pid"; then
  cat "$hover_server_log" "$hover_client_log"
  fatal 'hover_GPU_KMS_original_CRTC_restore_failed'
fi
kill "$hover_server_tail_pid" "$hover_client_tail_pid" 2>/dev/null || true
wait "$hover_server_tail_pid" "$hover_client_tail_pid" 2>/dev/null || true
for marker in \
  'MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true' \
  'MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true' \
  'MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true' \
  'NATIVE_SESSION_READY=false'; do
  grep -qx "$marker" "$hover_server_log" ||
    fatal "hover_virtual_GPU_rollback_missing_$marker"
done
rm -f "$hover_server_log" "$hover_client_log"
report 'MELANIA_QEMU_HOVER_FOCUS_REAL_WAYLAND_PASS=true'
report 'MELANIA_NATIVE_SESSION_READY=false'

# Stage136: REAL USB Super+S/Shift+S/Y -> XDG group-size serials ->
# two independent Wayland wl_shm commits -> one atomic CRTC repaint.
# Every selected tab is checked through actual QMP framebuffer pixels.
# The combined mode is opt-in only in this isolated disposable QEMU VT.
combined_server_log="$(mktemp -t melania-combined-server-XXXXXX)"
combined_client_log="$(mktemp -t melania-combined-client-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_GPU_WAYLAND_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_GPU_WAYLAND=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_DYNAMIC_SCENE=YES_QEMU_VM \
MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM \
MELANIA_NATIVE_TAB_STACKS=YES_QEMU_VM \
MELANIA_NATIVE_AUTO_TILING=YES_QEMU_VM \
MELANIA_NATIVE_COMBINED_TAB_TILING=YES_QEMU_VM \
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$combined_server_log" 2>&1 &
combined_server_pid=$!
combined_socket=''
for attempt in $(seq 1 100); do
  combined_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$combined_server_log" | head -n 1)"
  [[ -n "$combined_socket" ]] && break
  kill -0 "$combined_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$combined_socket" ]] ||
   ! grep -q '^MELANIA_NATIVE_IN_PROCESS_KMS_WORKER_STARTED=true' "$combined_server_log"; then
  cat "$combined_server_log"
  kill -TERM "$combined_server_pid" 2>/dev/null || true
  fatal 'combined_tab_real_KMS_guest_start_failed'
fi
tail -n +1 -f -s 0.05 "$combined_server_log" > /dev/ttyS0 &
combined_server_tail_pid=$!
tail -n +1 -f -s 0.05 "$combined_client_log" > /dev/ttyS0 &
combined_client_tail_pid=$!
report 'MELANIA_QEMU_COMBINED_TAB_STAGE_STARTED=true'
if ! MELANIA_QEMU_CONTINUOUS_TWO_CLIENTS=YES_QEMU_VM \
      MELANIA_QEMU_COMBINED_TAB_CLIENT=YES_QEMU_VM \
      WAYLAND_DISPLAY="$combined_socket" \
      timeout --signal=TERM --kill-after=3s 175s \
      "$tab_client" > "$combined_client_log" 2>&1; then
  cat "$combined_client_log" "$combined_server_log"
  kill -TERM "$combined_server_pid" 2>/dev/null || true
  fatal 'combined_tab_real_Wayland_client_failed'
fi
for marker in \
  'MELANIA_QEMU_COMBINED_TAB_TWO_REAL_XDG_ACKS=true' \
  'MELANIA_QEMU_COMBINED_TAB_TWO_OWNED_SHM_CALLBACKS=true' \
  'MELANIA_QEMU_COMBINED_TAB_TWO_REAL_KEYBOARD_ENTERS_PASS=true'; do
  grep -qx "$marker" "$combined_client_log" ||
    fatal "combined_tab_real_client_missing_$marker"
done
for marker in \
  'MELANIA_NATIVE_TAB_STACKING=true id=1' \
  'MELANIA_NATIVE_TAB_ATTACHED=1:2' \
  'MELANIA_NATIVE_AUTO_TILING_ENABLED=true' \
  'MELANIA_NATIVE_COMBINED_TILING_ARMED=1 clients=2' \
  'MELANIA_NATIVE_TAB_RESIZE_ATOMIC_SCENE_READY=1 clients=2' \
  'MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=14' \
  'MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=15' \
  'MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=16'; do
  grep -qx "$marker" "$combined_server_log" ||
    fatal "combined_tab_real_server_missing_$marker"
done
if grep -q '^MELANIA_NATIVE_COMBINED_TILING_TIMEOUT=' "$combined_server_log"; then
  fatal 'combined_tab_xdg_or_pixel_transaction_timed_out'
fi
kill -TERM "$combined_server_pid"
if ! wait "$combined_server_pid"; then
  cat "$combined_server_log" "$combined_client_log"
  fatal 'combined_tab_real_GPU_CRTC_restore_failed'
fi
kill "$combined_server_tail_pid" "$combined_client_tail_pid" 2>/dev/null || true
wait "$combined_server_tail_pid" "$combined_client_tail_pid" 2>/dev/null || true
for marker in \
  'MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true' \
  'MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true' \
  'MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true' \
  'NATIVE_SESSION_READY=false'; do
  grep -qx "$marker" "$combined_server_log" ||
    fatal "combined_tab_kernel_rollback_missing_$marker"
done
rm -f "$combined_server_log" "$combined_client_log"
report 'MELANIA_QEMU_NATIVE_COMBINED_TAB_TILING_REAL_CRTC_PASS=true'
report 'MELANIA_NATIVE_SESSION_READY=false'

# Stage118: real XDG parent + child popup mapped on the disposable
# one-process GPU/GBM/DRM path. QMP must capture both populated and cleared
# CRTC pixels, never a mock or configure-only image.
real_popup_client=/mnt/melania-native/melania-real-popup-client
[[ -x "$real_popup_client" ]] || fatal 'real_popup_test_client_missing'
popup_server_log="$(mktemp -t melania-native-popup-server-XXXXXX)"
popup_client_log="$(mktemp -t melania-native-popup-client-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_GPU_WAYLAND_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_PERSISTENT_GPU_WAYLAND=YES_DISPOSABLE_QEMU \
MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_DYNAMIC_SCENE=YES_QEMU_VM \
MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM \
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$popup_server_log" 2>&1 &
popup_server_pid=$!
popup_socket=''
for attempt in $(seq 1 100); do
  popup_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$popup_server_log" | head -n 1)"
  [[ -n "$popup_socket" ]] && break
  kill -0 "$popup_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$popup_socket" ]] ||
   ! grep -q '^MELANIA_NATIVE_IN_PROCESS_KMS_WORKER_STARTED=true' "$popup_server_log"; then
  cat "$popup_server_log"
  kill -TERM "$popup_server_pid" 2>/dev/null || true
  fatal 'real_popup_GPU_Wayland_server_not_ready'
fi
tail -n +1 -f -s 0.05 "$popup_server_log" > /dev/ttyS0 &
popup_server_tail_pid=$!
tail -n +1 -f -s 0.05 "$popup_client_log" > /dev/ttyS0 &
popup_client_tail_pid=$!
report 'MELANIA_QEMU_REAL_XDG_POPUP_STAGE118_STARTED=true'
if ! MELANIA_QEMU_XDG_POPUP_CLIENT=YES_QEMU_VM \
      MELANIA_QEMU_XDG_POPUP_GRAB=YES_QEMU_VM WAYLAND_DISPLAY="$popup_socket" \
   timeout --signal=TERM --kill-after=3s 100s \
   "$real_popup_client" > "$popup_client_log" 2>&1; then
  cat "$popup_client_log" "$popup_server_log"
  kill -TERM "$popup_server_pid" 2>/dev/null || true
  fatal 'real_xdg_popup_client_failed'
fi
for index in 1 2 3; do
  grep -q "^MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=$index$" "$popup_server_log" ||
    fatal "real_popup_kernel_CRTC_frame$index""_absent"
done
for index in 1 2; do
  grep -q "^MELANIA_NATIVE_REAL_POPUP_CLIENT_FRAME$index""_ACK=true$" "$popup_client_log" ||
    fatal "real_popup_client_frame$index""_callback_absent"
done
grep -q '^MELANIA_NATIVE_XDG_POPUP_CLIENT_BUFFER_ACCEPTED=180x100$' "$popup_server_log" ||
  fatal 'real_popup_client_owned_buffer_not_imported'
grep -q '^MELANIA_NATIVE_REAL_POPUP_CONFIGURE_ACK=280,115 180x100$' "$popup_client_log" ||
  fatal 'real_popup_xdg_positioner_ACK_not_received'
grep -q '^MELANIA_NATIVE_REAL_POPUP_CLIENT_DESTROYED=true$' "$popup_client_log" ||
  fatal 'real_popup_client_never_destroyed_role'
grep -q '^MELANIA_NATIVE_REAL_POPUP_CLIENT_POPUP_DONE_AFTER_ESCAPE=true$' "$popup_client_log" ||
  fatal 'real_popup_client_did_not_receive_popup_done'
grep -q '^MELANIA_NATIVE_XDG_POPUP_ESCAPE_DISMISSED=2$' "$popup_server_log" ||
  fatal 'real_usb_escape_did_not_dismiss_focused_popup'
kill -TERM "$popup_server_pid"
if ! wait "$popup_server_pid"; then
  cat "$popup_server_log" "$popup_client_log"
  fatal 'real_popup_GPU_KMS_restore_failed'
fi
kill "$popup_server_tail_pid" "$popup_client_tail_pid" 2>/dev/null || true
wait "$popup_server_tail_pid" "$popup_client_tail_pid" 2>/dev/null || true
for marker in \
  'MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true' \
  'MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true' \
  'MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true' \
  'NATIVE_SESSION_READY=false'; do
  grep -qx "$marker" "$popup_server_log" ||
    fatal "real_popup_kernel_restore_missing_$marker"
done
rm -f "$popup_server_log" "$popup_client_log"
report 'MELANIA_QEMU_REAL_XDG_POPUP_KMS3_PASS=true'
report 'MELANIA_NATIVE_SESSION_READY=false'

# Stage105: an actual ONE-PROCESS QEMU DRM/Wayland compositor must remain
# alive after its 24th real client frame. SIGTERM triggers orderly stream
# EOF, verified original CRTC/connector rollback and libseat device release.
# No receiver frame budget or compositor frame target is permitted.
persistent_server_log="$(mktemp -t melania-persistent-server-XXXXXX)"
persistent_client_log="$(mktemp -t melania-persistent-client-XXXXXX)"
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$persistent_server_log" 2>&1 &
persistent_server_pid=$!
persistent_socket=''
for attempt in $(seq 1 100); do
  persistent_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$persistent_server_log" | head -n 1)"
  [[ -n "$persistent_socket" ]] && break
  kill -0 "$persistent_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$persistent_socket" ]] ||
   ! grep -q '^MELANIA_NATIVE_PERSISTENT_KMS_STARTED=true' "$persistent_server_log"; then
  cat "$persistent_server_log"
  kill -TERM "$persistent_server_pid" 2>/dev/null || true
  fatal 'persistent_integrated_Smithay_KMS_process_never_started'
fi
if ! MELANIA_QEMU_CONTINUOUS_CLIENT=YES_QEMU_VM \
  MELANIA_QEMU_CONTINUOUS_CLIENT_FRAMES=24 WAYLAND_DISPLAY="$persistent_socket" \
  timeout --signal=TERM --kill-after=3s 100s \
  "$continuous_client" > "$persistent_client_log" 2>&1; then
  cat "$persistent_server_log" "$persistent_client_log"
  kill -TERM "$persistent_server_pid" 2>/dev/null || true
  fatal 'persistent_Wayland_client_failed'
fi
# A fixed-length receiver would have exited after frame 24. A receiver
# retaining its historical 12-second read timeout would also terminate
# during this deliberate idle gap. The real Wayland/KMS server must survive.
sleep 15
if ! kill -0 "$persistent_server_pid" 2>/dev/null; then
  cat "$persistent_server_log" "$persistent_client_log"
  fatal 'persistent_Smithay_exited_at_twenty_four_frames'
fi
report 'MELANIA_QEMU_PERSISTENT_IDLE_OVER_12_SECONDS_PASS=true'
kill -TERM "$persistent_server_pid"
if ! wait "$persistent_server_pid"; then
  cat "$persistent_server_log" "$persistent_client_log"
  fatal 'persistent_sigterm_CRTC_rollback_or_libseat_release_failed'
fi
for index in $(seq 1 24); do
  grep -q "^MELANIA_RUST_CONTINUOUS_FRAME_ACKED=$index " "$persistent_server_log" ||
    fatal "persistent_kernel_frame_${index}_not_acknowledged"
  grep -q "^MELANIA_NATIVE_CONTINUOUS_KMS_ACK=$index " "$persistent_server_log" ||
    fatal "persistent_Wayland_frame_${index}_not_retired"
done
for marker in \
  'MELANIA_NATIVE_PERSISTENT_KMS_STARTED=true' \
  'MELANIA_RUST_KMS_FLIP_LEDGER_CONFIRMED=23' \
  'MELANIA_RUST_KMS_VBLANK_EVENT_ACK=24' \
  'MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED=24' \
  'MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED=true' \
  'MELANIA_RUST_KMS_RESTORE_VERIFIED=true' \
  'MELANIA_NATIVE_LIBSEAT_DRM_ACQUIRED=true' \
  'MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true' \
  'MELANIA_NATIVE_PERSISTENT_KMS_SHUTDOWN_OK=true' \
  'NATIVE_SESSION_READY=false'; do
  grep -q "^$marker$" "$persistent_server_log" || {
    cat "$persistent_server_log"
    fatal "persistent_KMS_missing_${marker}"
  }
done
grep -Eq '^MELANIA_RUST_PERSISTENT_KMS_STOPPED_FRAMES=[0-9]+$' "$persistent_server_log" ||
  fatal 'persistent_receiver_missing_orderly_EOF_count'
grep -q '^MELANIA_NATIVE_CONTINUOUS_CLIENT_EXTENDED_FRAMES_PASS=24$' "$persistent_client_log" ||
  fatal 'persistent_client_missed_real_ack'
rm -f "$persistent_server_log" "$persistent_client_log"
report 'MELANIA_QEMU_IN_PROCESS_PERSISTENT_KMS_SIGTERM_ROLLBACK_PASS=true'
report 'MELANIA_NATIVE_SESSION_READY=false'

# Stage48: two INDEPENDENT xdg_toplevel surfaces alternate twelve real
# wl_shm commits. Every one requires real KMS FB_ID, frame.done and release.
# The final QMP screendump must show BOTH retained client images.
two_continuous_client=/mnt/melania-native/melania-two-continuous-client
[[ -x "$two_continuous_client" ]] || fatal 'two_window_continuous_client_missing'
two_server_log="$(mktemp -t melania-continuous-two-server-XXXXXX)"
two_kms_log="$(mktemp -t melania-continuous-two-kms-XXXXXX)"
two_client_log="$(mktemp -t melania-continuous-two-client-XXXXXX)"
rm -f /run/melania-native-qemu/continuous-frame-stream.sock
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 79s \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$two_server_log" 2>&1 &
two_server_pid=$!
two_socket=''
for attempt in $(seq 1 100); do
  two_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$two_server_log" | head -n 1)"
  [[ -n "$two_socket" && -S /run/melania-native-qemu/continuous-frame-stream.sock ]] && break
  kill -0 "$two_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$two_socket" || ! -S /run/melania-native-qemu/continuous-frame-stream.sock ]]; then
  cat "$two_server_log"
  fatal 'continuous_two_Wayland_bridge_unavailable'
fi
# A guest-to-serial tail exposes the live KMS activation to the host's QMP
# watcher BEFORE the receiver retires this framebuffer and restores the CRTC.
tail -n +1 -f -s 0.05 "$two_kms_log" > /dev/ttyS0 &
two_tail_pid=$!
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_CONTINUOUS_FRAMES=12 \
MELANIA_QEMU_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 72s \
  "$continuous_kms" < /dev/tty1 > "$two_kms_log" 2>&1 &
two_kms_pid=$!
for attempt in $(seq 1 100); do
  grep -q '^MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true' "$two_kms_log" && break
  kill -0 "$two_kms_pid" 2>/dev/null || break
  sleep .1
done
if ! grep -q '^MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true' "$two_kms_log"; then
  cat "$two_kms_log" "$two_server_log"
  kill "$two_server_pid" "$two_kms_pid" 2>/dev/null || true
  fatal 'continuous_two_Rust_KMS_unavailable'
fi
if ! MELANIA_QEMU_CONTINUOUS_TWO_CLIENTS=YES_QEMU_VM WAYLAND_DISPLAY="$two_socket" \
  timeout --signal=TERM --kill-after=3s 68s \
  "$two_continuous_client" > "$two_client_log" 2>&1; then
  cat "$two_client_log" "$two_kms_log" "$two_server_log"
  kill "$two_server_pid" "$two_kms_pid" 2>/dev/null || true
  fatal 'two_real_continuous_Wayland_clients_failed'
fi
if ! wait "$two_kms_pid"; then
  cat "$two_kms_log" "$two_server_log"
  kill "$two_server_pid" 2>/dev/null || true
  fatal 'two_real_continuous_KMS_receiver_failed'
fi
if ! wait "$two_server_pid"; then
  cat "$two_server_log"
  fatal 'two_real_continuous_Smithay_failed'
fi
kill "$two_tail_pid" 2>/dev/null || true
wait "$two_tail_pid" 2>/dev/null || true
cat "$two_client_log" "$two_kms_log" "$two_server_log"
grep -q '^MELANIA_NATIVE_CONTINUOUS_TWO_XDG_CONFIGURED=true' "$two_client_log" ||
  fatal 'two_distinct_xdg_toplevels_not_configured'
for index in $(seq 1 12); do
  expected_window=A
  ((index%2==0)) && expected_window=B
  grep -q "^MELANIA_NATIVE_CONTINUOUS_TWO_WINDOW_ACKED=$index window=$expected_window$" "$two_client_log" ||
    fatal "two_window_callback_$index_missing"
  grep -q "^MELANIA_NATIVE_CONTINUOUS_KMS_ACK=$index " "$two_server_log" ||
    fatal "two_window_Rust_scheduler_ACK_$index_missing"
  grep -q "^MELANIA_RUST_CONTINUOUS_FRAME_ACKED=$index " "$two_kms_log" ||
    fatal "two_window_real_KMS_$index_missing"
done
grep -q '^MELANIA_NATIVE_CONTINUOUS_TWO_WINDOW_12_FRAMES_PASS=true' "$two_client_log" ||
  fatal 'two_window_client_12_commits_incomplete'
grep -q '^MELANIA_NATIVE_CONTINUOUS_WAYLAND_KMS_FRAMES_PASS=12' "$two_server_log" ||
  fatal 'two_window_Smithay_12_callbacks_incomplete'
grep -q '^MELANIA_RUST_CONTINUOUS_FRAMES_PASS=12' "$two_kms_log" ||
  fatal 'two_window_KMS_12_FBs_not_presented'
grep -q '^MELANIA_RUST_KMS_RESTORE_VERIFIED=true' "$two_kms_log" ||
  fatal 'two_window_original_CRTC_not_restored'
grep -q '^MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED=true' "$two_kms_log" ||
  fatal 'two_window_connector_route_not_restored'
grep -q '^MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED=12$' "$two_kms_log" ||
  fatal 'kernel_scanout_route_not_verified_two_kms_log_12'
rm -f "$two_server_log" "$two_kms_log" "$two_client_log"
report 'MELANIA_QEMU_NATIVE_CONTINUOUS_TWO_WINDOWS_PASS=true'

# Stage49: true xdg destruction after frame12, compositor-only close frame13,
# and a different xdg_toplevel in recycled slot1 on frame14. B must survive.
reopen_server_log="$(mktemp -t melania-continuous-reopen-server-XXXXXX)"
reopen_kms_log="$(mktemp -t melania-continuous-reopen-kms-XXXXXX)"
reopen_client_log="$(mktemp -t melania-continuous-reopen-client-XXXXXX)"
rm -f /run/melania-native-qemu/continuous-frame-stream.sock
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_REOPEN=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 85s \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$reopen_server_log" 2>&1 &
reopen_server_pid=$!
reopen_socket=''
for attempt in $(seq 1 100); do
  reopen_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$reopen_server_log" | head -n 1)"
  [[ -n "$reopen_socket" && -S /run/melania-native-qemu/continuous-frame-stream.sock ]] && break
  kill -0 "$reopen_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$reopen_socket" || ! -S /run/melania-native-qemu/continuous-frame-stream.sock ]]; then
  cat "$reopen_server_log"
  fatal 'continuous_reopen_Wayland_bridge_unavailable'
fi
# Serial marker is emitted while the virtual CRTC still scans out frames13/14.
tail -n +1 -f -s 0.05 "$reopen_kms_log" > /dev/ttyS0 &
reopen_tail_pid=$!
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_CONTINUOUS_FRAMES=14 \
MELANIA_QEMU_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_QEMU_CONTINUOUS_REOPEN=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 77s \
  "$continuous_kms" < /dev/tty1 > "$reopen_kms_log" 2>&1 &
reopen_kms_pid=$!
for attempt in $(seq 1 100); do
  grep -q '^MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true' "$reopen_kms_log" && break
  kill -0 "$reopen_kms_pid" 2>/dev/null || break
  sleep .1
done
if ! grep -q '^MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true' "$reopen_kms_log"; then
  cat "$reopen_kms_log" "$reopen_server_log"
  kill "$reopen_server_pid" "$reopen_kms_pid" 2>/dev/null || true
  fatal 'continuous_reopen_Rust_KMS_unavailable'
fi
if ! MELANIA_QEMU_CONTINUOUS_TWO_CLIENTS=YES_QEMU_VM \
     MELANIA_QEMU_CONTINUOUS_REOPEN_CLIENT=YES_QEMU_VM \
     WAYLAND_DISPLAY="$reopen_socket" \
  timeout --signal=TERM --kill-after=3s 72s \
  "$two_continuous_client" > "$reopen_client_log" 2>&1; then
  cat "$reopen_client_log" "$reopen_kms_log" "$reopen_server_log"
  kill "$reopen_server_pid" "$reopen_kms_pid" 2>/dev/null || true
  fatal 'continuous_real_close_reopen_client_failed'
fi
if ! wait "$reopen_kms_pid"; then
  cat "$reopen_kms_log" "$reopen_server_log"
  kill "$reopen_server_pid" 2>/dev/null || true
  fatal 'continuous_close_reopen_KMS_receiver_failed'
fi
if ! wait "$reopen_server_pid"; then
  cat "$reopen_server_log"
  fatal 'continuous_close_reopen_Smithay_failed'
fi
kill "$reopen_tail_pid" 2>/dev/null || true
wait "$reopen_tail_pid" 2>/dev/null || true
cat "$reopen_client_log" "$reopen_kms_log" "$reopen_server_log"
for index in $(seq 1 14); do
  grep -q "^MELANIA_NATIVE_CONTINUOUS_KMS_ACK=$index " "$reopen_server_log" ||
    fatal "continuous_reopen_scheduler_$index_not_acked"
  grep -q "^MELANIA_RUST_CONTINUOUS_FRAME_ACKED=$index " "$reopen_kms_log" ||
    fatal "continuous_reopen_CRTC_$index_not_verified"
done
grep -q '^MELANIA_NATIVE_CONTINUOUS_CLOSE_FRAME13_CALLBACK=true' "$reopen_server_log" ||
  fatal 'continuous_close_KMS_missing_survivor_callback'
grep -q '^MELANIA_NATIVE_CONTINUOUS_CLOSE_FRAME13_ACKED=true' "$reopen_client_log" ||
  fatal 'continuous_survivor_did_not_receive_callback'
grep -q '^MELANIA_NATIVE_CONTINUOUS_REOPEN_FRAME14_ACKED=true' "$reopen_client_log" ||
  fatal 'continuous_reopened_client_not_acknowledged'
grep -q '^MELANIA_NATIVE_CONTINUOUS_REOPEN_14_FRAMES_PASS=true' "$reopen_server_log" ||
  fatal 'continuous_Smithay_close_reopen_not_verified'
grep -q '^MELANIA_NATIVE_CONTINUOUS_REOPEN_14_FRAMES_PASS=true' "$reopen_client_log" ||
  fatal 'continuous_reopened_client_failed'
grep -q '^MELANIA_RUST_CONTINUOUS_FRAMES_PASS=14' "$reopen_kms_log" ||
  fatal 'continuous_KMS_14_frame_sequence_failed'
grep -q '^MELANIA_RUST_KMS_RESTORE_VERIFIED=true' "$reopen_kms_log" ||
  fatal 'continuous_close_reopen_original_CRTC_not_restored'
grep -q '^MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED=true' "$reopen_kms_log" ||
  fatal 'reopen_connector_route_not_restored'
grep -q '^MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED=14$' "$reopen_kms_log" ||
  fatal 'kernel_scanout_route_not_verified_reopen_kms_log_14'
rm -f "$reopen_server_log" "$reopen_kms_log" "$reopen_client_log"
report 'MELANIA_QEMU_NATIVE_CONTINUOUS_REOPEN_14_FRAMES_PASS=true'

# Stage53: TWO distinct xdg_toplevels with different wl_shm dimensions
# (A 320x192, B 220x180) through generic owned import, retained scene,
# Rust continuous KMS ACK and a real QMP framebuffer pixel-boundary test.
dynamic_server_log="$(mktemp -t melania-dynamic-server-XXXXXX)"
dynamic_kms_log="$(mktemp -t melania-dynamic-kms-XXXXXX)"
dynamic_client_log="$(mktemp -t melania-dynamic-client-XXXXXX)"
rm -f /run/melania-native-qemu/continuous-frame-stream.sock
rm -f /run/melania-native-qemu/dynamic-pointer-frame13-acked
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_DYNAMIC_SCENE=YES_QEMU_VM \
MELANIA_NATIVE_XDG_RESIZE=YES_QEMU_VM \
MELANIA_NATIVE_DYNAMIC_POINTER_PROOF=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 83s \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$dynamic_server_log" 2>&1 &
dynamic_server_pid=$!
dynamic_socket=''
for attempt in $(seq 1 100); do
  dynamic_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$dynamic_server_log" | head -n 1)"
  [[ -n "$dynamic_socket" && -S /run/melania-native-qemu/continuous-frame-stream.sock ]] && break
  kill -0 "$dynamic_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$dynamic_socket" || ! -S /run/melania-native-qemu/continuous-frame-stream.sock ]]; then
  cat "$dynamic_server_log"
  fatal 'dynamic_scene_real_Wayland_bridge_unavailable'
fi
tail -n +1 -f -s 0.05 "$dynamic_kms_log" > /dev/ttyS0 &
dynamic_tail_pid=$!
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_CONTINUOUS_FRAMES=13 \
MELANIA_QEMU_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_QEMU_CONTINUOUS_DYNAMIC=YES_QEMU_VM \
MELANIA_QEMU_CONTINUOUS_DYNAMIC_POINTER=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 75s \
  "$continuous_kms" < /dev/tty1 > "$dynamic_kms_log" 2>&1 &
dynamic_kms_pid=$!
for attempt in $(seq 1 100); do
  grep -q '^MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true' "$dynamic_kms_log" && break
  kill -0 "$dynamic_kms_pid" 2>/dev/null || break
  sleep .1
done
if ! grep -q '^MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true' "$dynamic_kms_log"; then
  cat "$dynamic_kms_log" "$dynamic_server_log"
  kill "$dynamic_server_pid" "$dynamic_kms_pid" 2>/dev/null || true
  fatal 'dynamic_scene_real_KMS_receiver_unavailable'
fi
if ! MELANIA_QEMU_CONTINUOUS_TWO_CLIENTS=YES_QEMU_VM \
     MELANIA_QEMU_CONTINUOUS_DYNAMIC_CLIENT=YES_QEMU_VM \
     MELANIA_QEMU_XDG_RESIZE_CLIENT=YES_QEMU_VM \
     MELANIA_QEMU_DYNAMIC_POINTER_CLIENT=YES_QEMU_VM \
     WAYLAND_DISPLAY="$dynamic_socket" \
  timeout --signal=TERM --kill-after=3s 68s \
  "$two_continuous_client" > "$dynamic_client_log" 2>&1; then
  cat "$dynamic_client_log" "$dynamic_kms_log" "$dynamic_server_log"
  kill "$dynamic_server_pid" "$dynamic_kms_pid" 2>/dev/null || true
  fatal 'different_sized_real_Wayland_clients_failed'
fi
if ! wait "$dynamic_kms_pid"; then
  cat "$dynamic_kms_log" "$dynamic_server_log"
  kill "$dynamic_server_pid" 2>/dev/null || true
  fatal 'different_sized_real_KMS_presenter_failed'
fi
if ! wait "$dynamic_server_pid"; then
  cat "$dynamic_server_log"
  fatal 'different_sized_Smithay_compositor_failed'
fi
kill "$dynamic_tail_pid" 2>/dev/null || true
wait "$dynamic_tail_pid" 2>/dev/null || true
cat "$dynamic_client_log" "$dynamic_kms_log" "$dynamic_server_log"
grep -q '^MELANIA_NATIVE_CONTINUOUS_DYNAMIC_SIZES=320x192;220x180' "$dynamic_client_log" ||
  fatal 'dynamic_client_sizes_missing'
grep -q '^MELANIA_NATIVE_CONTINUOUS_DYNAMIC_SHRINK=400x240->320x192;280x200->220x180' "$dynamic_client_log" ||
  fatal 'dynamic_client_shrink_sequence_missing'
grep -q '^MELANIA_NATIVE_DYNAMIC_TWO_SIZES_12_KMS_FRAMES_PASS=true' "$dynamic_client_log" ||
  fatal 'dynamic_client_real_callbacks_missing'
grep -q '^MELANIA_NATIVE_DYNAMIC_PREMULTIPLIED_ARGB_CLIENT_PASS=true' "$dynamic_client_log" ||
  fatal 'dynamic_real_premultiplied_Wayland_ARGB_missing'
grep -q '^MELANIA_NATIVE_DYNAMIC_XDG_RESIZE_CONFIGURES_SENT=true' "$dynamic_server_log" ||
  fatal 'compositor_real_xdg_resize_configures_missing'
grep -q '^MELANIA_NATIVE_CLIENT_XDG_RESIZE_ACK_A=true' "$dynamic_client_log" ||
  fatal 'client_A_real_xdg_resize_ack_missing'
grep -q '^MELANIA_NATIVE_CLIENT_XDG_RESIZE_ACK_B=true' "$dynamic_client_log" ||
  fatal 'client_B_real_xdg_resize_ack_missing'
grep -q '^MELANIA_NATIVE_DYNAMIC_POINTER_CLIENT_STAYED_ALIVE=true' "$dynamic_client_log" ||
  fatal 'resized_wayland_clients_disconnected_before_real_pointer_KMS'
for index in $(seq 1 13); do
  grep -q "^MELANIA_NATIVE_CONTINUOUS_KMS_ACK=$index " "$dynamic_server_log" ||
    fatal "dynamic_KMS_scheduler_ACK_$index_missing"
  grep -q "^MELANIA_RUST_CONTINUOUS_FRAME_ACKED=$index " "$dynamic_kms_log" ||
    fatal "dynamic_real_CRTC_frame_$index_missing"
done
grep -q '^MELANIA_NATIVE_CONTINUOUS_WAYLAND_KMS_FRAMES_PASS=13' "$dynamic_server_log" ||
  fatal 'dynamic_Smithay_thirteen_frame_KMS_missing'
grep -q '^MELANIA_NATIVE_DYNAMIC_POINTER_KMS13_ACKED=true' "$dynamic_server_log" ||
  fatal 'dynamic_pointer_geometry_or_real_KMS_ACK_missing'
grep -q '^MELANIA_NATIVE_DYNAMIC_POINTER_FRAME13_PASS=true' "$dynamic_server_log" ||
  fatal 'dynamic_pointer_compositor_only_redraw_missing'
grep -q '^MELANIA_RUST_CONTINUOUS_DYNAMIC_FRAME_VISIBLE=12' "$dynamic_kms_log" ||
  fatal 'dynamic_QMP_capture_frame_missing'
grep -q '^MELANIA_RUST_CONTINUOUS_DYNAMIC_POINTER_FRAME_VISIBLE=13' "$dynamic_kms_log" ||
  fatal 'resized_mouse_drag_never_reached_real_CRTC'
grep -q '^MELANIA_RUST_CONTINUOUS_FRAMES_PASS=13' "$dynamic_kms_log" ||
  fatal 'dynamic_KMS_thirteen_frames_not_presented'
grep -q '^MELANIA_RUST_KMS_RESTORE_VERIFIED=true' "$dynamic_kms_log" ||
  fatal 'dynamic_real_KMS_original_CRTC_not_restored'
grep -q '^MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED=true' "$dynamic_kms_log" ||
  fatal 'dynamic_connector_route_not_restored'
grep -q '^MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED=13$' "$dynamic_kms_log" ||
  fatal 'kernel_scanout_route_not_verified_dynamic_kms_log_13'
rm -f "$dynamic_server_log" "$dynamic_kms_log" "$dynamic_client_log"
report 'MELANIA_QEMU_NATIVE_DYNAMIC_TWO_SIZES_PASS=true'
report 'MELANIA_QEMU_NATIVE_DYNAMIC_POINTER_KMS13_PASS=true'

# Stage81B: different test, same REAL Smithay -> owned wl_shm -> retained
# scene -> Rust continuous DRM/KMS CRTC ACK pipeline. The client changes A's
# xdg role at frames 3/5/7/9 (maximize/restore/fullscreen/restore) and must
# commit correctly-sized buffers while B remains a separate live toplevel.
# This proves CRTC frame acknowledgements, not yet role-specific QMP pixels.
managed_server_log="$(mktemp -t melania-managed-role-server-XXXXXX)"
managed_kms_log="$(mktemp -t melania-managed-role-kms-XXXXXX)"
managed_client_log="$(mktemp -t melania-managed-role-client-XXXXXX)"
rm -f /run/melania-native-qemu/continuous-frame-stream.sock
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_DYNAMIC_SCENE=YES_QEMU_VM \
MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM \
MELANIA_NATIVE_MANAGED_MINIMIZE_QMP=YES_QEMU_VM \
MELANIA_NATIVE_QEMU_FRAME_TARGET=14 \
  timeout --signal=TERM --kill-after=3s 103s \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$managed_server_log" 2>&1 &
managed_server_pid=$!
managed_socket=''
for attempt in $(seq 1 100); do
  managed_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$managed_server_log" | head -n 1)"
  [[ -n "$managed_socket" && -S /run/melania-native-qemu/continuous-frame-stream.sock ]] && break
  kill -0 "$managed_server_pid" 2>/dev/null || break
  sleep .1
done
managed_control_socket="$(sed -n 's/^MELANIA_NATIVE_CONTROL_SOCKET_READY=//p' "$managed_server_log" | head -n 1)"
if [[ -z "$managed_socket" || ! -S /run/melania-native-qemu/continuous-frame-stream.sock ||
      -z "$managed_control_socket" || ! -S "$managed_control_socket" ]]; then
  cat "$managed_server_log"
  fatal 'managed_role_live_Wayland_stream_unavailable'
fi
# The host cannot inspect restored or replayed frames: stream each live
# KMS-visible marker to serial while its framebuffer is still active.
tail -n +1 -f -s 0.05 "$managed_kms_log" > /dev/ttyS0 &
managed_tail_pid=$!
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_CONTINUOUS_FRAMES=14 \
MELANIA_QEMU_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_QEMU_CONTINUOUS_MANAGED_ROLE_QMP=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 96s \
  "$continuous_kms" < /dev/tty1 > "$managed_kms_log" 2>&1 &
managed_kms_pid=$!
for attempt in $(seq 1 100); do
  grep -q '^MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true' "$managed_kms_log" && break
  kill -0 "$managed_kms_pid" 2>/dev/null || break
  sleep .1
done
if ! grep -q '^MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true' "$managed_kms_log"; then
  cat "$managed_server_log" "$managed_kms_log"
  kill "$managed_server_pid" "$managed_kms_pid" 2>/dev/null || true
  fatal 'managed_role_KMS_worker_not_connected'
fi
if ! MELANIA_QEMU_CONTINUOUS_TWO_CLIENTS=YES_QEMU_VM \
       MELANIA_QEMU_MANAGED_ROLE_CLIENT=YES_QEMU_VM \
       MELANIA_QEMU_MANAGED_MINIMIZE_CLIENT=YES_QEMU_VM \
       MELANIA_CONTROL_SOCKET="$managed_control_socket" \
       WAYLAND_DISPLAY="$managed_socket" \
  timeout --signal=TERM --kill-after=3s 86s \
  "$two_continuous_client" > "$managed_client_log" 2>&1; then
  cat "$managed_client_log" "$managed_kms_log" "$managed_server_log"
  kill "$managed_server_pid" "$managed_kms_pid" 2>/dev/null || true
  fatal 'managed_role_real_Wayland_client_failed'
fi
if ! wait "$managed_kms_pid"; then
  cat "$managed_kms_log" "$managed_server_log"
  kill "$managed_server_pid" 2>/dev/null || true
  fatal 'managed_role_KMS_CRTC_12_ACKs_failed'
fi
if ! wait "$managed_server_pid"; then
  cat "$managed_server_log"
  fatal 'managed_role_Smithay_live_stream_failed'
fi
kill "$managed_tail_pid" 2>/dev/null || true
wait "$managed_tail_pid" 2>/dev/null || true
cat "$managed_client_log" "$managed_kms_log" "$managed_server_log"
for index in 3 5 7 9 13 14; do
  grep -q "^MELANIA_RUST_CONTINUOUS_MANAGED_ROLE_FRAME_VISIBLE=$index$" "$managed_kms_log" ||
    fatal "managed_role_real_CRTC_QMP_capture_marker_$index_missing"
done
for index in $(seq 1 12); do
  grep -q "^MELANIA_NATIVE_MANAGED_ROLE_KMS_CALLBACK=$index$" "$managed_client_log" ||
    fatal "managed_role_client_KMS_callback_$index_missing"
  grep -q "^MELANIA_NATIVE_CONTINUOUS_KMS_ACK=$index " "$managed_server_log" ||
    fatal "managed_role_retained_scene_CRTC_ACK_$index_missing"
  grep -q "^MELANIA_RUST_CONTINUOUS_FRAME_ACKED=$index " "$managed_kms_log" ||
    fatal "managed_role_real_DRM_CRTC_$index_missing"
done
for transition in '3 size=1280x800' '5 size=360x224' \
                  '7 size=1280x800' '9 size=360x224'; do
  grep -q "^MELANIA_NATIVE_MANAGED_ROLE_CLIENT_ACK=$transition$" "$managed_client_log" ||
    fatal "managed_role_client_configure_ACK_missing:$transition"
done
for event in \
  'MELANIA_NATIVE_MANAGED_XDG_MAXIMIZED=true' \
  'MELANIA_NATIVE_MANAGED_XDG_MAXIMIZED=false' \
  'MELANIA_NATIVE_MANAGED_XDG_FULLSCREEN=true' \
  'MELANIA_NATIVE_MANAGED_XDG_FULLSCREEN=false'; do
  grep -q "^$event$" "$managed_server_log" ||
    fatal "managed_role_Smithay_request_missing:$event"
done
grep -q '^MELANIA_NATIVE_MANAGED_ROLE_12_KMS_FRAMES_PASS=true' "$managed_client_log" ||
  fatal 'managed_role_client_did_not_complete_all_real_CRTC_ACKs'
grep -q '^MELANIA_NATIVE_CONTINUOUS_WAYLAND_KMS_FRAMES_PASS=14' "$managed_server_log" ||
  fatal 'managed_role_compositor_did_not_complete_fourteen_frames'
grep -q '^MELANIA_RUST_CONTINUOUS_FRAMES_PASS=14' "$managed_kms_log" ||
  fatal 'managed_role_KMS_did_not_complete_fourteen_frames'
for index in 13 14; do
  grep -q "^MELANIA_NATIVE_MANAGED_MINIMIZE_KMS_ACK=$index " "$managed_server_log" ||
    fatal "managed_role_compositor_minimize_restore_ACK_$index_missing"
  grep -q "^MELANIA_RUST_CONTINUOUS_FRAME_ACKED=$index " "$managed_kms_log" ||
    fatal "managed_role_CRTC_minimize_restore_ACK_$index_missing"
done
grep -q '^MELANIA_NATIVE_MANAGED_MINIMIZE_CLIENT_KMS13_PASS=true' "$managed_client_log" ||
  fatal 'client_did_not_observe_KMS_hidden_frame'
grep -q '^MELANIA_NATIVE_MANAGED_MINIMIZE_CLIENT_KMS14_PASS=true' "$managed_client_log" ||
  fatal 'client_did_not_observe_KMS_restored_frame'
grep -q '^MELANIA_RUST_KMS_RESTORE_VERIFIED=true' "$managed_kms_log" ||
  fatal 'managed_role_did_not_restore_original_CRTC'
grep -q '^MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED=true' "$managed_kms_log" ||
  fatal 'managed_connector_route_not_restored'
grep -q '^MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED=14$' "$managed_kms_log" ||
  fatal 'kernel_scanout_route_not_verified_managed_kms_log_14'
rm -f "$managed_server_log" "$managed_kms_log" "$managed_client_log"
report 'MELANIA_QEMU_MANAGED_XDG_12_REAL_KMS_ACKS_PASS=true'
report 'MELANIA_QEMU_MANAGED_MINIMIZE_RESTORE_14_KMS_ACKS_PASS=true'

# Stage82D: two genuine Wayland roles share one Rust KMS stream.
# The xdg toplevel sends odd frames; bottom Top-layer panel sends evens.
layer_client=/mnt/melania-native/melania-layer-panel-client
[[ -x "$layer_client" ]] || fatal 'real_layer_panel_client_missing'
layer_server_log="$(mktemp -t melania-real-layer-server-XXXXXX)"
layer_client_log="$(mktemp -t melania-real-layer-client-XXXXXX)"
layer_kms_log="$(mktemp -t melania-real-layer-kms-XXXXXX)"
rm -f /run/melania-native-qemu/continuous-frame-stream.sock
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM \
MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM \
MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM \
MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_NATIVE_DYNAMIC_SCENE=YES_QEMU_VM \
MELANIA_NATIVE_LAYER_PIXELS=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 78s \
  "$continuous_server" --experimental-wayland-seat < /dev/tty1 \
  > "$layer_server_log" 2>&1 &
layer_server_pid=$!
layer_socket=''
for attempt in $(seq 1 100); do
  layer_socket="$(sed -n 's/^MELANIA_NATIVE_WAYLAND_SOCKET_READY=//p' "$layer_server_log" | head -n 1)"
  [[ -n "$layer_socket" && -S /run/melania-native-qemu/continuous-frame-stream.sock ]] && break
  kill -0 "$layer_server_pid" 2>/dev/null || break
  sleep .1
done
if [[ -z "$layer_socket" || ! -S /run/melania-native-qemu/continuous-frame-stream.sock ]]; then
  cat "$layer_server_log"
  fatal 'real_layer_wayland_socket_missing'
fi
# The QMP host monitor must observe the framebuffer BEFORE CRTC restore.
# tail -f may drop the final marker when its polling/flush loses the race
# against the 2.5-second KMS scanout hold. A named-pipe tee forwards each
# actual Rust presenter log write directly to the guest serial port while
# preserving a complete file for kernel route and rollback verification.
# This NEVER synthesizes a frame-12 marker or weakens QMP pixel assertions.
layer_kms_forward_dir="$(mktemp -d -t melania-layer-serial-XXXXXX)"
layer_kms_fifo="$layer_kms_forward_dir/frames.fifo"
mkfifo -m 600 "$layer_kms_fifo"
tee "$layer_kms_log" < "$layer_kms_fifo" > /dev/ttyS0 &
layer_tail_pid=$!
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_CONTINUOUS_FRAMES=12 \
MELANIA_QEMU_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_QEMU_REAL_LAYER_PANEL=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 72s \
  "$continuous_kms" < /dev/tty1 > "$layer_kms_fifo" 2>&1 &
layer_kms_pid=$!
for attempt in $(seq 1 100); do
  grep -q '^MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true' "$layer_kms_log" && break
  kill -0 "$layer_kms_pid" 2>/dev/null || break
  sleep .1
done
if ! grep -q '^MELANIA_RUST_CONTINUOUS_KMS_CONNECTED=true' "$layer_kms_log"; then
  cat "$layer_server_log" "$layer_kms_log"
  kill "$layer_server_pid" "$layer_kms_pid" 2>/dev/null || true
  fatal 'real_layer_KMS_worker_missing'
fi
if ! MELANIA_QEMU_LAYER_PANEL_CLIENT=YES_QEMU_VM WAYLAND_DISPLAY="$layer_socket" \
  timeout --signal=TERM --kill-after=3s 66s "$layer_client" \
  > "$layer_client_log" 2>&1; then
  cat "$layer_client_log" "$layer_kms_log" "$layer_server_log"
  kill "$layer_server_pid" "$layer_kms_pid" 2>/dev/null || true
  fatal 'real_layer_client_failed'
fi
if ! wait "$layer_kms_pid"; then
  cat "$layer_kms_log" "$layer_server_log"
  kill "$layer_server_pid" 2>/dev/null || true
  fatal 'real_layer_CRTC_worker_failed'
fi
if ! wait "$layer_server_pid"; then
  cat "$layer_server_log"
  fatal 'real_layer_Smithay_failed'
fi
# Ensure all KMS markers have reached /dev/ttyS0 before cleanup.
# If the stream forwarder failed, QMP could not have observed live pixels.
if ! wait "$layer_tail_pid"; then
  cat "$layer_kms_log" "$layer_server_log"
  fatal 'real_layer_serial_forwarder_failed'
fi
rm -f "$layer_kms_fifo"
rmdir "$layer_kms_forward_dir"
cat "$layer_client_log" "$layer_kms_log" "$layer_server_log"
grep -q '^MELANIA_NATIVE_REAL_LAYER_AND_XDG_CONFIGURED=true' "$layer_client_log" ||
  fatal 'real_xdg_and_layer_roles_not_configured'
for index in $(seq 1 12); do
  role=xdg
  ((index%2==0)) && role=layer
  grep -q "^MELANIA_NATIVE_REAL_LAYER_KMS_CALLBACK=$index role=$role$" "$layer_client_log" ||
    fatal "real_layer_callback_$index_missing"
  grep -q "^MELANIA_NATIVE_CONTINUOUS_KMS_ACK=$index " "$layer_server_log" ||
    fatal "real_layer_Smithay_KMS_ack_$index_missing"
  grep -q "^MELANIA_RUST_CONTINUOUS_FRAME_ACKED=$index " "$layer_kms_log" ||
    fatal "real_layer_CRTC_frame_$index_missing"
done
grep -q '^MELANIA_RUST_CONTINUOUS_LAYER_FRAME_VISIBLE=12' "$layer_kms_log" ||
  fatal 'real_layer_QMP_frame12_marker_absent'
grep -q '^MELANIA_NATIVE_LAYER_OWNED_SHM_QUEUED=1280x52 layer=Top' "$layer_server_log" ||
  fatal 'real_layer_owned_client_pixels_not_imported'
grep -q '^MELANIA_NATIVE_REAL_LAYER_12_KMS_FRAMES_PASS=true' "$layer_client_log" ||
  fatal 'real_layer_client_frames_incomplete'
grep -q '^MELANIA_RUST_KMS_RESTORE_VERIFIED=true' "$layer_kms_log" ||
  fatal 'real_layer_original_CRTC_not_restored'
grep -q '^MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED=true' "$layer_kms_log" ||
  fatal 'real_layer_connector_route_not_restored'
grep -q '^MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED=12$' "$layer_kms_log" ||
  fatal 'kernel_scanout_route_not_verified_layer_kms_log_12'
rm -f "$layer_client_log" "$layer_kms_log" "$layer_server_log"
report 'MELANIA_QEMU_REAL_LAYER_SHELL_12_KMS_PASS=true'

report 'MELANIA_NATIVE_SESSION_READY=false'
report 'MELANIA_NATIVE_VISIBLE_SCANOUT=false'
report 'MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false'
