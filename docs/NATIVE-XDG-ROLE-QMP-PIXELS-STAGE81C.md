# Native Stage 81C — actual virtual-display pixels during xdg transitions

This builds on Stage 81B's **real** Wayland `xdg_toplevel` configure/ACK,
client-owned variable-size `wl_shm` buffer, retained-scene damage and
Rust DRM/KMS CRTC FB_ID verification.

The disposable VM Rust KMS receiver emits uniquely gated, live serial
markers while the framebuffer is active at frames 3, 5, 7 and 9. The
host's independent QMP monitor captures the GPU's `screendump` at each
marker and compares the PPM bytes to the expected colors of the **actual
client buffers**. It checks:
- maximize at frame 3: A fills the output, B remains above A
- restore at frame 5: A returns to its normal bounds; old pixels vanish
- fullscreen at frame 7: A fills the output while B remains mapped
- exit fullscreen at frame 9: A returns to its original normal bounds
- border pixels and background outside restored A; B retains its own
  independent previous client image throughout

A missing marker, screenshot, mismatched pixel, KMS ACK or failure to
restore the original CRTC fails the entire GitHub QEMU workflow. The
ordinary Linux tests use **synthetic byte fixtures only to test the
verifier's comparisons**; they are never presented as a real screenshot.

This is a QEMU **integration test**, not a native login-ready desktop.
No user-facing visual changes, host DRM use, VT takeover, production
session activation or login authorization. `NATIVE_SESSION_READY=false`.
