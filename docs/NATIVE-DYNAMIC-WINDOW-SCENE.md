# Stage 52 — retained dynamic window geometry

The real two-window QEMU experiment has proven multiple independent
wl_shm commits, visible physical CRTC scanout and close/reopen. Its original
scene is deliberately fixed to 1280×800 output and two 360×224 windows, so
its successful CI does not mean a usable native window manager.

Stage52 introduces a separate \`DynamicWindowScene\` Rust model with:
- Up to sixteen independently owned and resized client images (not 16
  working desktop apps yet).
- Variable output dimensions within a 64 MiB output allocation limit,
  window dimensions up to 8192×8192 within a 64 MiB per-image limit, and a
  96 MiB total retained client-pixel budget.
- Stable compositor-assigned window IDs, arbitrary signed positions and
  output clipping, active z-order (raise), hit testing, close/reopen and
  output resize with generation invalidation.
- Proper wl_shm ARGB8888 **premultiplied** alpha blending. The previously
  available StraightAlphaBgra path remains unchanged; untrusted components
  that exceed their declared alpha are clamped.
- Source code and Rust unit tests for window retention, different client
  sizes, resize, focus/z order, clear-on-close, alpha and invalid state.

The generic scene uses the existing CPU \`scene::compose\` rasterization
primitive without taking ownership of DRM, Wayland or libseat handles.
It never advertises \`NATIVE_SESSION_READY\` and does not change Start,
panel, themes, Melania Files, icons, or titlebar buttons.

**Not yet integrated:** the native Smithay Wayland bridge continues using
the fixed-size two-window QEMU scene. Its graphical CI stays unchanged.
After Stage51 generic \`owned_shm\` is integrated, connect its validated
variable-size client pixels to this scene; then prove true differently
sized \`xdg_toplevel\` windows on actual QEMU KMS scanout, followed by
multi-output GBM/GLES and VT/device-lifecycle integration. Only then
consider native display-manager login.
