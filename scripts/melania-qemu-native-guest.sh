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
if [[ -z "$shell_socket" ]]; then
  cat "$shell_log"
  kill "$shell_pid" 2>/dev/null || true
  wait "$shell_pid" 2>/dev/null || true
  fatal 'native_xdg_layer_socket_unavailable'
fi
if ! MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM WAYLAND_DISPLAY="$shell_socket" \
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
  timeout --signal=TERM --kill-after=3s 83s \
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
if [[ -z "$managed_socket" || ! -S /run/melania-native-qemu/continuous-frame-stream.sock ]]; then
  cat "$managed_server_log"
  fatal 'managed_role_live_Wayland_stream_unavailable'
fi
# The host cannot inspect restored or replayed frames: stream each live
# KMS-visible marker to serial while its framebuffer is still active.
tail -n +1 -f -s 0.05 "$managed_kms_log" > /dev/ttyS0 &
managed_tail_pid=$!
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_CONTINUOUS_FRAMES=12 \
MELANIA_QEMU_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_QEMU_CONTINUOUS_MANAGED_ROLE_QMP=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 76s \
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
       WAYLAND_DISPLAY="$managed_socket" \
  timeout --signal=TERM --kill-after=3s 70s \
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
for index in 3 5 7 9; do
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
grep -q '^MELANIA_NATIVE_CONTINUOUS_WAYLAND_KMS_FRAMES_PASS=12' "$managed_server_log" ||
  fatal 'managed_role_compositor_did_not_complete_twelve_frames'
grep -q '^MELANIA_RUST_CONTINUOUS_FRAMES_PASS=12' "$managed_kms_log" ||
  fatal 'managed_role_KMS_did_not_complete_twelve_frames'
grep -q '^MELANIA_RUST_KMS_RESTORE_VERIFIED=true' "$managed_kms_log" ||
  fatal 'managed_role_did_not_restore_original_CRTC'
grep -q '^MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED=true' "$managed_kms_log" ||
  fatal 'managed_connector_route_not_restored'
grep -q '^MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED=12$' "$managed_kms_log" ||
  fatal 'kernel_scanout_route_not_verified_managed_kms_log_12'
rm -f "$managed_server_log" "$managed_kms_log" "$managed_client_log"
report 'MELANIA_QEMU_MANAGED_XDG_12_REAL_KMS_ACKS_PASS=true'

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
tail -n +1 -f -s 0.05 "$layer_kms_log" > /dev/ttyS0 &
layer_tail_pid=$!
MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU \
MELANIA_QEMU_CONTINUOUS_FRAMES=12 \
MELANIA_QEMU_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM \
MELANIA_QEMU_REAL_LAYER_PANEL=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 72s \
  "$continuous_kms" < /dev/tty1 > "$layer_kms_log" 2>&1 &
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
kill "$layer_tail_pid" 2>/dev/null || true
wait "$layer_tail_pid" 2>/dev/null || true
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
