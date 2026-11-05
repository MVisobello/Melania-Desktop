# Stage 110: signal-terminated persistent GPU-backed Wayland

This extends the Stage109 **real Wayland client pixels → GLES
ImportMem → GBM DMA-BUF → DRM KMS** pipeline beyond 24 frames.

Only an explicitly opted-in disposable QEMU guest on a dedicated Linux
VT/libseat session may select this output backend. The existing finite
24-frame GPU proof remains unchanged, and CPU dumb-buffer fallback stays
the default for non-GPU QEMU tests.

## Runtime implementation

- No fixed frame limit in the persistent mode: the receiver continues
  accepting versioned, generation-checked scene frames until **clean EOF
  at a frame boundary**.
- No 12-second idle socket read deadline in that mode. A compositor with
  unchanged client surfaces does not need to repaint, even for hours.
- Two real GPU DMA-BUFs and their DRM framebuffer IDs are retained and
  reused throughout the output lifetime. The renderer waits for GPU sync,
  kernel pageflip events, per-CRTC routing and the live framebuffer/
  connector/encoder/CRTC check before sending each Wayland frame ACK.
- Signal handling remains in the Smithay/libseat owner. SIGTERM/SIGINT
  stops new submissions, drains the pending KMS job, closes the sender
  transport, joins the real DRM reader which then verifies CRTC and
  connector restoration, and **only then** releases the GPU from libseat.
- A GPU/pageflip/read/protocol failure aborts the session instead of
  returning a successful Wayland callback or silently reverting to CPU.

QEMU now performs real client scene updates for 48 frames, verifies each
actual GPU presentation and client ACK, pauses 15 seconds after frame 24,
resumes to frame 48, then checks another 15-second idle, SIGTERM shutdown,
kernel CRTC rollback and libseat release. The end-to-end client watchdog
allows 240 seconds for 48 actual page flips under TCG software emulation.
The previous 105-second watchdog demonstrably expired after 46 successful
kernel-confirmed frames in CI; this change does not waive any ACK, frame,
rollback, or per-frame validation requirement.

## Why COSMIC remains the architectural reference

The per-GPU resource owner, per-output renderer, drm-rs pageflip routing,
Wayland presentation callbacks and session lifecycle are separate
responsibilities. Melania builds these abstractions in original Rust,
using Smithay's APIs; no GPL cosmic-comp implementation is imported.

## What still prevents a production-native login

The persistent proof drives one 1280×800 VirtIO CRTC. Rendering uploads a
CPU-composited scene to GPU, not individual client dmabufs. Work remains
for atomic multi-output GBM swapchains, hotplug/VT reacquisition,
buffer damage and direct scanout, Xwayland/IME, secure session locking,
screen capture permissions and recovery from crashes/driver loss.

Do not enable a display-manager native session or claim readiness:
`NATIVE_SESSION_READY=false`. Keep labwc as the stable backend.
