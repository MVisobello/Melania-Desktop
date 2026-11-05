# Stage 103 — kernel page-flip/vblank event gating in Melania Native

## Motivation

The existing QEMU one-process Wayland compositor and GBM scanout probe already
submitted real DRM frames and confirmed the CRTC framebuffer ID. However,
**polling the CRTC framebuffer alone** does not establish that the asynchronous
page flip has completed, nor when the old buffer is safe to recycle. A modern
Smithay/KMS compositor treats the kernel vblank/page-flip completion as part
of its per-output frame lifecycle.

## Concrete implementation

- `native_drm_flip::wait_for_kms_page_flip` waits using a bounded DRM FD poll,
  consumes `drm-rs` events and accepts a `PageFlip` event for the
  **expected CRTC** only.
- The existing isolated QEMU KMS receiver now submits
  `PageFlipFlags::EVENT` on every asynchronous flip. The receiver accepts
  no event-free success path: it requires matching kernel completion **and**
  its existing live CRTC framebuffer + connector/encoder verification before
  emitting `MLA1` and releasing any client Wayland frame callbacks.
- The separate actual GPU/GBM/EGL two-frame QEMU probe uses the same helper
  before its live framebuffer check.
- A kernel timeout, FD error/HUP or wrong CRTC is fatal; the existing rollback
  guard still attempts restoring the original CRTC **before** releasing
  KMS framebuffer/GBM resources and before the owner closes libseat DRM.
- CI source contracts and the QEMU guest's real kernel log markers require
  evidence of at least the second GBM flip and the twelfth integrated
  compositor flip.

This preserves every previously verified QMP pixel and CRTC route probe and
does not replace the active compositor on the host.

## Architecture/licensing

Inspired by COSMIC's output-event-driven architecture and Smithay's
`DrmDeviceNotifier` design, without importing or copying GPL-3.0-only
COSMIC source. All implementation here is original MIT Melania Rust.

## Explicit limits

The current QEMU tests still use a bounded number of frames and a 1280×800
one-CRTC output. This PR does **not** add a persistent DRM/GBM swapchain,
atomic output configuration, multi-GPU routing, device reacquisition or safe
VT resume. These remain essential before Melania Native can replace labwc.
`NATIVE_SESSION_READY=false` remains unchanged. Merge only after Linux
builds, Rust tests and actual QEMU DRM tests all succeed.
