# Melania Native — Stage 104: per-output kernel flip sequence ledger

## Purpose

Real DRM page-flip completion now feeds a **monotonic, per-CRTC Rust ledger** on
both of Melania's experimental DRM render paths. This makes the future
event-driven continuous output lifecycle harder to accidentally regress.

The old code could accept any `PageFlip` event for the matching CRTC.
A delayed or duplicated kernel event on that output would be insufficient
evidence that the *latest* queued submission completed. Stage 104 adds a
single-outstanding-flip reservation and a validated 32-bit kernel vblank
counter before an asynchronous frame becomes eligible for framebuffer
verification and Wayland client acknowledgement.

### Concrete wiring

- `NativeFlipLedger::begin_flip` reserves the one pending submission
  immediately before the real `card.page_flip(..., PageFlipFlags::EVENT)`.
- `native_drm_flip::wait_for_kms_page_flip` now returns the kernel's actual
  `PageFlipEvent.frame` for the correct CRTC rather than a bare `()`.
- `NativeFlipLedger::complete_flip` only accepts a strictly forward
  counter; wraparound is supported, but duplicate/backward/ambiguous jumps
  permanently revoke this resource instance.
- The receiver and real GBM/EGL renderer both require the ledger success,
  **then** the existing actual kernel FB_ID/connector-route check. Only the
  final proof may authorize the ordinary Wayland presentation receipt.
- Real QEMU guest checks now require kernel sequence ledger markers for both
  the GBM second frame and integrated Wayland 12th frame.
- The ledger has no fixed 12/64-frame ceiling, no GPU ownership shortcuts,
  and no capacity for concurrent flips on one output. Tests exercise 4,096
  sequential frames as well as wrong/duplicated events and 32-bit wrap.

## Boundaries

This does *not* turn the current 12/24-frame VM proof into a persistent
production DRM event loop. It does not yet own an atomic GBM swapchain,
multi-CRTC output resources, DRM master recovery or actual VT resume.
All experimental hardware operations still run only with the original
QEMU/TTY1/explicit opt-in guards. The production Melania desktop continues
to use the labwc fallback and `NATIVE_SESSION_READY=false`.

COSMIC informs the per-output, kernel-event architecture only; all Rust here
is original Melania MIT code, not copied from GPL COSMIC sources.
