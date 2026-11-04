# Melania — first single-process Wayland + DRM/KMS guest milestone

**Goal:** stop relying on an independent DRM/KMS receiver process to
present client pixels in the disposable QEMU session. Prove an actual
`melania-native` executable can run both the Smithay Wayland server and
Rust DRM/KMS scanout worker concurrently.

## Integration path

- Existing `--experimental-wayland-seat` starts the real Smithay display
  on a dedicated QEMU `/dev/tty1`.
- Opt-in `MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM` (alongside the existing
  QEMU-only continuous-KMS flags) creates a local `UnixStream::pair()`.
- The same executable spawns the **existing verified Rust KMS receiver**
  in a thread. No second `melania-continuous-kms` process is launched.
- Actual Wayland wl_shm client commits become scene images; the bounded
  worker forwards those images to the local KMS thread and waits for its
  kernel-verified CRTC/connector presentation ACK.
- The final successful path **joins the DRM worker**, so it cannot report
  a passing one-process test before the original framebuffer and connector
  route have been restored.
- The disposable QEMU guest runs the normal 12-frame real Wayland client,
  requiring twelve client callbacks, twelve real kernel frame ACKs, the
  shared DRM scanout route verifier and original CRTC rollback.

## Reproduce (disposable QEMU guest only)

Use `scripts/melania-qemu-native-guest.sh` through the native QEMU GitHub
Actions workflow. The script requires `/dev/tty1`, DMI proving QEMU/KVM
and the explicit environment variables used by the existing VM test.

Pass condition:
`MELANIA_QEMU_IN_PROCESS_WAYLAND_KMS_12_FRAMES_PASS=true`.

The test cannot pass merely from an in-memory Rust simulation: the
Wayland client process is separate and sends genuine wl_shm commits,
the server submits them to the real VirtIO DRM card and only acknowledges
frames after kernel state inspection.

## What remains before a production compositing session

This is an **integration checkpoint, not the final native backend**.
The worker still uses the QEMU-only dumb-buffer DRM presenter and a
fixed 12-frame test scenario. It does not yet acquire the DRM card FD
through the *same* libseat session as Smithay; it does not create and
own primary KMS planes with GBM/EGL, handle full atomic rollback, support
multiple outputs or survive a real VT pause/resume. On any pause, the
experimental server still fails closed. There is no display-manager
registration and **no hardware readiness claim**.

Next acceptance test: a long-running, dynamically sized GBM scanout
loop using libseat-managed device ownership and full VT rollback on
dedicated hardware. `NATIVE_SESSION_READY=false`.
