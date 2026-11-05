# Melania Native — Stage 106: per-CRTC DRM event routing

The previous stage reads a DRM event batch and immediately returns on the
first page-flip event belonging to the requested CRTC. This is correct
only for one CRTC. On a GPU driving multiple outputs it can drop a pending
event belonging to another CRTC when both arrive in the same read.

## Implemented in Rust

`DrmFlipEventRouter` is a bounded, per-device-epoch event queue. It:

- fully drains every batch returned by drm-rs `receive_events()`, retaining
  page-flip completions belonging to *other* CRTCs instead of discarding them;
- lets each output consume only the completion for its expected CRTC;
- rejects duplicate completions for the same CRTC in the queue and refuses
  more than 16 unconsumed events, permanently revoking the bad epoch;
- revokes on FD error, event read failure, HUP or deadline expiration;
- supports consecutive frames without an artificial frame-count limit.

The actual `qemu-rust-kms-continuous` compositor's DRM renderer and the
actual GBM/EGL KMS two-frame renderer now both use this shared module. Their
independent per-output monotonic vblank ledgers (Stage 104) and live kernel
framebuffer/connector routing checks (Stage 96) remain required before
acknowledging a client frame.

Rust tests inject interleaved completions for three CRTCs, duplicate events,
queue overflow and recreation of a clean device epoch. QEMU tests continue
exercising real kernel events, including 24 integrated Wayland/KMS frames.

## Important limitations

This is **event routing**, not automatic multi-monitor modesetting. The
current integrated QEMU output remains a single 1280×800 VirtIO CRTC with
two dumb buffers. A production backend still needs multiple native output
objects, connector/encoder/mode routing, per-CRTC GBM swapchains, atomic
pageflip commits, VT/DRM master recovery and physical hardware tests.

PageFlipEvent in drm-rs 0.14 exposes CRTC ID and frame counter, but not an
end-to-end scene ticket. Therefore this router never claims that the event
alone proves presentation; the existing monotonic ledger, current libseat
generation and live kernel FB_ID/route verification are all still mandatory.

`NATIVE_SESSION_READY=false` remains unchanged. The regular Melania
desktop continues to use labwc; no host login backend was modified.
