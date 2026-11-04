# Stage 91 — extended continuous Wayland/KMS frame proof (QEMU only)

The current native compositor proof used to stop after 12, 13 or 14
frames, depending on the scenario. The underlying DRM/KMS receiver already
supports a validated 8..=64-frame protocol; Stage91 exposes its capacity
to the real Smithay Wayland bridge and to the real wl_shm client without
altering the default 12-frame regression.

## New controlled environment variables

These settings are **experimental and QEMU-only**; they cannot enable a login
session and must be used only with all existing dedicated-VM flags and TTY
requirements.

- `MELANIA_NATIVE_QEMU_FRAME_TARGET=24` on the Smithay bridge
- `MELANIA_QEMU_CONTINUOUS_CLIENT_FRAMES=24` on the Wayland client
- `MELANIA_QEMU_CONTINUOUS_FRAMES=24` on the Rust DRM/KMS receiver (existing)

The three counts must match. The reusable Rust parser accepts only decimal
12..=64, rejecting invalid, negative, overflowed, nonnumeric or out-of-budget
inputs rather than silently falling back to twelve frames.

The default path still generates twelve client-owned buffers, waits for
their actual DRM CRTC verification, acknowledges their matching generation
and sequence, then returns all framebuffer resources to the original KMS
state. The VM-specific input/seat revocation path from Stage89 remains
mandatory.

## Validation

- `cargo test -p melania-native --all-targets` exercises the pure Rust
  frame-budget parser and verifies rejection of unsafe limits.
- `tests/test_native_extended_frames_stage91.py` verifies the bridge,
  client and receiver use the same bounded contract.
- The unchanged existing QEMU 12-frame proof remains the mandatory
  end-to-end non-regression. To **demonstrate actual 24-frame operation**
  a dedicated real VM run must set all three variables and verify twenty-four
  distinct KMS ACKs, frame callbacks and the final original CRTC restoration.

No arbitrary-length running session, automatic VT recovery, multi-output
hardware ownership, or login support is claimed.
`NATIVE_SESSION_READY=false`.
