# Stage 39 — resized client pixels, fifth KMS frame, real QMP screenshot

This stage extends the Stage38 xdg-shell resize handshake into a **new,
real client-owned wl_shm buffer of the configured size**. The QEMU client
waits for the server's verified fourth layout-only KMS ACK, then attaches
its new 386×246 XRGB8888 buffer to the original xdg_toplevel, commits and
waits for its genuine frame callback and wl_buffer.release.

The server imports the new size and pixels within Smithay's protected
wl_shm mapping, checks format, offset, stride, dimensions and bounds,
and retains only an **owned copy**. The two-window scene now stores
per-window width/height; window A's resizing does not affect B.

The async Rust DRM worker presents a **fifth page-flip**, after the
original A/B/A commits and layout-only drag frame. The guest then restores
the original CRTC before acknowledging the fifth frame. A QMP screenshot
must prove that actual client pixels occupy newly expanded coordinates
that were background during Stage37, while window B stays visible.

## Acceptance and limits

- All additional operations are explicitly gated by
  `MELANIA_NATIVE_RESIZED_BUFFER=YES_QEMU_VM` in a disposable QEMU VM.
- The old 3/4-frame paths retain their previous strict bounds.
- The client must receive and ACK the real xdg configure, then submit a
  new-size buffer; a synthetic resize snapshot cannot satisfy this gate.
- A fifth screenshot and frame ACK are required, not just log strings.
- No continuous multi-monitor compositor, GPU-accelerated compositing,
  hardware-seat recovery or desktop-manager login is enabled.
- `MELANIA_NATIVE_SESSION_READY=false` remains mandatory.

The temporary readiness file is a **QEMU test synchronization gate**
between the already ACKed fourth frame and the fifth client commit. It
is not an API for production clients and does not replace Wayland
configure/ACK rules.
