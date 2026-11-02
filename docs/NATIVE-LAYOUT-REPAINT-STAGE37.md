# Stage 37 — real mouse drag → independent Rust KMS repaint (QEMU-only)

## Result intended

Stage36 changed compositor geometry without immediately changing the display
framebuffer. Stage37 connects a *real* QEMU USB-tablet drag to Smithay's
retained two-window scene and schedules an **independent fourth KMS frame**
even if the Wayland client sends no additional `wl_shm` commit.

The first three KMS frames are still produced by true `xdg_toplevel`
buffers from two real Wayland surfaces (A, B, A). The fourth is composed
only from the compositor-owned copies of these buffers, after the layout
moves window A by approximately (+80, +70). A fresh frame ticket is used;
the async worker remains off the Smithay event loop, and its verified KMS
ACK occurs after the original CRTC is restored. No fake buffer.release()
or frame callback is emitted for this layout-only frame.

## Safety and acceptance

- `MELANIA_NATIVE_LAYOUT_REPAINT=YES_QEMU_VM` gates this entire extension;
  the legacy QEMU three-frame path remains unchanged.
- Frame ordinals are bounded at four only under the explicit VM flag; the
  worker rejects invalid frame sizes and ACK mismatches.
- The libinput drag is injected through QEMU's private QMP socket, never
  through any input or display device on the host.
- The fourth QMP PPM screenshot must prove pixels of A shifted, background
  visible at A's old position, and B still retained. Merely looking for
  progress text is insufficient.
- Tests fail closed if screenshot validation, DRM/CRTC restoration, frame
  ticket completion, QMP input delivery or previous three-frame checks fail.
- `MELANIA_NATIVE_SESSION_READY=false`: no physical GPU login support.

## Remaining limitations

The renderer is still a bounded proof, **not** a continuous compositor.
It does not yet support arbitrary output sizes, clients, z-order repaint,
sustained dragging at refresh rate, GPU acceleration, multi-GPU or robust
session switching. Resize in Stage36 is still a preview because changing
actual client dimensions needs `xdg_toplevel.configure`, client ACK,
and an imported `wl_shm` buffer of the newly negotiated size.

The VM KMS gate, if successful, establishes real *visual* position
changes, not production readiness.
