# Melania Native — Stage 108: real GBM framebuffer reuse on a kernel CRTC

This stage follows the **per-output rendering and buffer lifetime** pattern
found in mature Rust/Smithay compositors such as COSMIC Comp. It is original
Melania code; it does not copy COSMIC's GPL implementation.

## Implemented

The isolated QEMU renderer now has two actual long-lived GBM DMA-BUF
allocation objects, each imported as a real DRM `FB_ID`, and reuses them
across **twelve successive GPU-rendered and KMS-presented frames**.

`native_gbm_swapchain.rs` tracks each real CRTC independently:

- reserve only a slot that is NOT currently being scanned by the output;
- reject another reservation while a presentation is pending;
- after the actual kernel page-flip event (or first synchronous modeset)
  **and** the verified connector/CRTC/framebuffer route, mark a new slot
  as visible;
- reject wrong-output/wrong-FB confirmations, permanently revoking the
  unsafe generation until fresh resource acquisition;
- allow independent state for different CRTCs, although simultaneous
  multi-output KMS remains to be implemented.

The GBM renderer synchronizes each actual GLES frame, passes the two
retained kernel FB_ID objects to `set_crtc` or `page_flip(EVENT)`, waits
for the DRM vblank and verifies the live connector and kernel FB_ID.
Only AFTER those checks does it mark a slot as successfully presented.

The 12-frame test has a **separate explicit opt-in**
`MELANIA_NATIVE_VM_GBM_REUSE=YES_DISPOSABLE_QEMU`; it additionally
requires the existing QEMU DMI/tty1 check, KMS opt-in, libseat, and an
isolated single-output VirtIO GPU. It leaves the original two-frame
scenario unchanged.

Both end by restoring the original CRTC, connector and framebuffer
*before* dropping either DMA-BUF or kernel FB_ID and releasing libseat.
The real VM guest verifies twelve visible frames, eleven kernel
page-flip acknowledgements, ten framebuffer reuses, and successful
rollback. The host's final PASS gate requires the new result.

## What is NOT finished

This is NOT yet the GPU renderer for the main Wayland compositor:
the embedded real Wayland/KMS renderer still transfers wl_shm scenes
to two retained DRM dumb buffers. The next step is to bind the compositor
scene output to GPU render targets and then move to Smithay's actual
per-output DRM surface / atomic commit path. Further blockers are
multimonitor GPU outputs, VT suspend/resume, input, robust resource
reacquisition, and real hardware tests.

`NATIVE_SESSION_READY=false` remains mandatory. The normal Melania
desktop still uses labwc; no production host login or DRM mode is touched.
