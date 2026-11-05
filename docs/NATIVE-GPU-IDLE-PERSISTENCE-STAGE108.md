# Stage 108 — persistent DRM idle and actual reused GBM scanout

Melania continues its original Rust/Smithay native compositor development
with COSMIC Comp as an **architecture reference**, not copied GPL source.

## Bug fixed: idle compositor could quit after twelve seconds

The Stage105 in-process DRM receiver used the historical 12-second socket
read timeout even after the compositor became explicitly persistent.
Damage-driven Wayland compositors may have NO new image for hours: idle is
not a renderer failure. Now `MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU`
together with `MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM` disables only
this receiver's **between-frame** read deadline. Legacy finite QEMU tests
retain the 12-second timeout.

The existing worker shutdown path invokes socket shutdown, delivering EOF
even when the persistent reader is blocking on an idle stream; verified
CRTC/connector restoration still happens before libseat releases the card.

A real QEMU guest now delivers 24 actual Wayland/DRM frames, leaves the
compositor idle for **15 seconds**, confirms it is still alive, sends SIGTERM
and verifies normal CRTC rollback and device release.

## Real GPU-backed 24-frame stress

Previously the separate GBM/EGL scanout probe rendered two GPU-generated
DMA-BUFs and completed a single asynchronous DRM page flip.

The same real hardware-interface Rust path can now, under a separate
disposable-QEMU opt-in, render **24 output-sized GLES images through two
retained GBM DMA-BUFs and kernel framebuffer IDs**, alternating the reuse
target. No GPU buffers or framebuffer ID objects are allocated per frame.

For every asynchronous page flip it requires:
- real `PageFlipFlags::EVENT` completion on the chosen CRTC;
- bounded event receipt from the Stage106 per-CRTC router;
- the Stage104 outstanding-flip ledger (with explicit VirtIO VBlank quirk);
- verified current kernel FB_ID plus connector -> encoder -> CRTC route.

Only after each kernel scanout proof may the next frame reuse its
non-displayed DMA-BUF. On success *or* error the original CRTC restoration
runs before dropping the GBM framebuffer objects. The old two-frame QEMU
acceptance is retained and independently verified.

## Still missing for production

This is a standalone real-GPU stress experiment, **not** a full Wayland
compositor renderer. The integrated Wayland-to-KMS pixel path still uses
dumb buffers in QEMU. To replace labwc requires an integrated GPU-backed
Wayland renderer, per-output GBM/atomic swapchains, client buffer import,
full input/clipboard/IME/Xwayland, lock/security, robust hotplug and VT
pause/resume, display-manager startup and hardware QA.

`NATIVE_SESSION_READY=false` remains mandatory.
