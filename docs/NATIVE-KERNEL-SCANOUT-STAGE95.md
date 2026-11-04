# Stage 95 — kernel-backed DRM scanout acknowledgement

The Stage94 output dispatch coordinator previously had a public method
accepting `kernel_presented: bool`. A caller could incorrectly pass
`true` after an IPC ACK without checking the kernel CRTC. That is
unsafe as a public boundary.

Stage95 makes that boolean-only commit method **private** and introduces
`NativeOutputDispatch::verify_and_present`, taking a live drm-rs
`ControlDevice` plus expected connector, CRTC and framebuffer handles.

To retire a pending frame, the new public method checks:

1. The exact active resource and redraw ticket is still in flight.
2. Kernel `get_crtc` reports the expected framebuffer ID.
3. Kernel `get_connector` reports a live encoder.
4. Kernel `get_encoder` reports that encoder connected to the expected CRTC.
5. The existing per-frame watchdog has not expired.

Only then does the internal commit method return `true`, after which an
actual compositor may send `wl_surface.frame` and `wl_buffer.release`
callbacks. Stale submissions return false without probing or disturbing
the current frame. A kernel ioctl error or routing mismatch fails closed.

**Limits:** this is real drm-rs kernel-state verification code, not a
finished modeset loop. It is not yet called by a production compositor;
the QEMU proof still uses its existing dedicated Rust DRM receiver.
An actual libseat-owned, long-lived GPU backend must create and own
compatible framebuffer resources and deal with disconnects, VT releases,
buffer lifetime, and atomic rollback.

`NATIVE_SESSION_READY=false`. Do not expose it as a display-manager
session before hardware-backed integration and negative testing.
