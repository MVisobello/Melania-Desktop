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
MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM \
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
if ! WAYLAND_DISPLAY="$shell_socket" \
  timeout --signal=TERM --kill-after=3s 19s "$shell_client"; then
  cat "$shell_log"
  kill "$shell_pid" 2>/dev/null || true
  wait "$shell_pid" 2>/dev/null || true
  fatal 'real_xdg_layer_configure_ack_failed'
fi
if ! wait "$shell_pid"; then
  cat "$shell_log"
  fatal 'native_wayland_shell_server_failed'
fi
cat "$shell_log"
grep -q '^MELANIA_NATIVE_SHELL_PROTOCOLS_READY=true' "$shell_log" ||
  fatal 'native_xdg_layer_shell_server_not_configured'
grep -q '^MELANIA_NATIVE_SHELL_RENDERED=false' "$shell_log" ||
  fatal 'native_wayland_shell_claimed_graphical_rendering'
rm -f "$shell_log"
report 'MELANIA_QEMU_NATIVE_SHELL_PROBE_PASS=true'

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
MELANIA_NATIVE_LAYOUT_REPAINT=YES_QEMU_VM \
MELANIA_NATIVE_RESIZE_CONFIGURE=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 39s \
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
MELANIA_NATIVE_LAYOUT_REPAINT=YES_QEMU_VM \
  timeout --signal=TERM --kill-after=3s 38s \
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
  MELANIA_NATIVE_LAYOUT_REPAINT=YES_QEMU_VM \
  MELANIA_NATIVE_RESIZE_CONFIGURE=YES_QEMU_VM WAYLAND_DISPLAY="$multi_socket" \
  timeout --signal=TERM --kill-after=3s 31s "$pixel_client"; then
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
grep -q '^MELANIA_NATIVE_LAYOUT_REPAINT_SUBMITTED=4' "$multi_server_log" ||
  fatal 'drag_scene_did_not_submit_fourth_KMS_frame'
grep -q '^MELANIA_NATIVE_LAYOUT_REPAINT_ACK=4' "$multi_server_log" ||
  fatal 'drag_scene_fourth_KMS_frame_not_acknowledged'
grep -q '^MELANIA_RUST_LAYOUT_REPAINT_FOUR_FRAMES_PASS=true' "$multi_kms_log" ||
  fatal 'DRM_never_presented_fourth_dragged_scene_frame'
grep -q '^MELANIA_RUST_KMS_RESTORE_VERIFIED=true' "$multi_kms_log" ||
  fatal 'two_window_KMS_original_CRTC_not_restored'
for index in 1 2 3 4; do
  grep -q "^MELANIA_NATIVE_SCHEDULED_FRAME_ACK=$index$" "$multi_server_log" ||
    fatal "two_window_frame_ticket_$index_not_retired"
done
rm -f "$multi_server_log" "$multi_kms_log"
report 'MELANIA_QEMU_TWO_REAL_WINDOWS_COMPOSITED_PASS=true'
report 'MELANIA_QEMU_NONBLOCKING_KMS_WORKER_PASS=true'
report 'MELANIA_QEMU_LAYOUT_ONLY_KMS_REPAINT_PASS=true'
report 'MELANIA_QEMU_XDG_RESIZE_PROBE_ENABLED=true'
report 'MELANIA_NATIVE_CONTINUOUS_COMPOSITOR_READY=false'

report 'MELANIA_NATIVE_SESSION_READY=false'
report 'MELANIA_NATIVE_VISIBLE_SCANOUT=false'
report 'MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false'
