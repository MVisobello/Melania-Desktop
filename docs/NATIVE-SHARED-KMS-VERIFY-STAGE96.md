# Stage 96 — shared kernel scanout verifier exercised in QEMU

Stage95 added a real DRM CRTC/encoder verification path to the future native
output dispatch ledger, but it was not exercised by the existing QEMU
hardware proof. Stage96 removes that separation.

## Implementation

The shared `melania-native/src/native_kms_scanout.rs` function accepts a
live drm-rs control device and the expected **CRTC, connector and framebuffer**.
It performs kernel reads of CRTC FB_ID and connector encoder routing. It
returns false for missing/mismatched hardware state, and returns an error on
DRM ioctl failure.

Both the Stage95 `NativeOutputDispatch::verify_and_present` and the
**actual Rust QEMU DRM receiver** now call this same helper. The receiver
continues to wait for visible CRTC framebuffer scanout and additionally
requires the shared connector/CRTC check before writing each MLA1 success ACK.
Its serial log prints
`MELANIA_RUST_KMS_SCANOUT_ROUTE_VERIFIED=<frame-number>`.

Eight VM scenarios explicitly require the marker on their respective final
frames (1, 3, 12, 12, 14, 13, 12 and 12).

## Evidence and limits

The QEMU workflow on this exact PR head must pass, including the existing
frame screenshot and rollback tests, to establish that the **same kernel
probe** is used on real virtual DRM presentation. Structural checks or
cargo compilation alone are not a hardware proof.

This neither creates a production DRM presenter nor owns a console VT.
It does not replace the future live libseat/GBM resource-acquisition and
atomic-rollback work. Do not register a login-manager session:
`NATIVE_SESSION_READY=false`.
