# Stage 89 — libseat/VT revocation for native KMS workers

## Delivered

The real (still **QEMU-only**) Smithay Wayland bridge now uses an epoch-bound
`NativePresentationGuard` shared across three critical paths:

1. Real libinput events are forwarded to Wayland clients only with an
   initially active libseat generation.
2. A queued continuous DRM/KMS scene carries a token minted for that seat
   generation, alongside its ordinary frame/scene ticket.
3. The asynchronous DRM FB_ID receipt can release `wl_shm` buffers or call
   `wl_surface.frame` only if the original token is *still* valid.

On `PauseSession`, the guard is revoked **before** libinput is suspended.
The QEMU bridge exits with a failed gate instead of processing more Wayland
events or accepting late KMS acknowledgements. A subsequent
`ActivateSession` cannot silently re-arm it; restoring output ownership
correctly requires real DRM/GBM framebuffer lifetime preservation, a VT
handoff and verified output reconfiguration, which are not yet implemented.

This removes a dangerous assumption from the native experiment: a KMS worker
may finish an ioctl *after* its compositor has lost the active seat.

## Verifications

- Rust unit tests for startup, seat pause, pending frame invalidation,
  attempted resume, and repeated revocation.
- QEMU workflow assertions that the live bridge implements this ordering,
  followed by its existing real Wayland+libinput, framebuffer/pixel, and
  isolation gates.
- QEMU guest restrictions (dedicated TTY, QEMU DMI and opt-in flags) remain
  unchanged.

## Explicit remaining blockers

This is **not** a fully operational native desktop compositor. Most
notably: no production per-GPU `DrmSurface` and continuously running
Smithay GBM renderer, no verified rollback of a borrowed KMS framebuffer or
VT, no safe resume/hotplug path, no complete general keyboard/pointer/touch
routing across GTK, Qt and XWayland, no production session manager, secure
lock or display-manager login.

`MELANIA_NATIVE_SESSION_READY=false`. Retain the existing Mint22 nested
preview as the user-testable environment until those prerequisites are
proven in isolated VMs and on supported real hardware.
