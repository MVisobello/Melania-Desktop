# Melania Native — COSMIC-informed backend audit and first integration

## Architectural reference (no imported GPL code)

COSMIC's compositor separates display/DRM devices, input/libseat lifecycle, output surfaces and a window-management shell. Smithay's public backend/session APIs provide comparable abstractions. The MIT-licensed Melania code must independently implement its own version of these ideas: cosmic-comp itself is GPL-3.0-only, so we do **not** copy its implementation or mix its source directly into this MIT project.

References:
- https://github.com/pop-os/cosmic-comp/tree/master/src/backend/kms
- https://github.com/pop-os/cosmic-comp/blob/master/src/backend/kms/device.rs
- https://smithay.github.io/smithay/smithay/backend/session/index.html

## What changed in Melania

The experimental Wayland bridge already processed libseat PauseSession/ActivateSession but did **not** subscribe to the seat's DRM udev notifications. If a GPU/connector disappeared or changed between a client commit and a KMS ACK, the bridge had no explicit hotplug invalidation.

This change:
1. Creates one real Smithay UdevBackend for the exact libseat seat used by the Wayland bridge, before creating the KMS presenter.
2. Captures an initial device-ID/path map to distinguish an unchanged enumerated card from an added, removed, changed or replaced card.
3. Registers the udev backend as another source on the existing calloop event loop, alongside libseat and libinput — not a polling thread.
4. On an actual GPU change, revokes the NativePresentationGuard epoch **before** dropping pending KMS transports and client presentation tokens; invalidates the NativeLifecycle frame gate and the redraw scheduler; cancels input interactions and clears the partial-composition baseline.
5. The next event-loop iteration exits the isolated bridge through its existing error-cleanup path. An integrated presenter must finish/attempt CRTC rollback and be joined before releasing the card via libseat, rather than continuing to submit to a removed/replaced GPU.
6. Rust tests verify a duplicate initial udev enumeration is harmless, but new/replaced/removed/changed devices cannot restore an old epoch. The QEMU KMS workflow must still pass without any test injection.

## Remaining blockers — prioritized by production impact

**A. A real persistent DRM/GBM backend:** current QEMU bridge still has a finite frame target and an external or experimental integrated presenter. A real compositor needs persistent per-output DRM surfaces and page-flip events instead of a fixed 12/14-frame capture.

**B. GPU/VT/suspend recovery:** currently any GPU change or seat pause stops the QEMU experiment safely. For production, reactivation requires fresh libseat device ownership, rebuilding GBM/EGL/output/plane state, and verifying the original CRTC/connector baseline before any new frame; it cannot be assumed merely from ActivateSession.

**C. Native output management:** real output hotplug, multi-CRTC, mode selection, scaling and rotation must be bound to the Wayland output configuration protocol and output-specific render loops, not a fixed advertised 1280×800 virtual output.

**D. Compatibility and session protocols:** Xwayland support, lock session, screencast/portal integration, clipboard/drag-drop, input-method/IME and secure login/logout all require tests as well as implementation. Use the labwc-backed Melania session as the functional fallback until these pass.

**E. Physical hardware testing:** disposable VM tests and GitHub unit tests do not prove a production-capable VT/DRM session on Intel/AMD/NVIDIA, multiple monitors or suspend/resume.

This change intentionally retains NATIVE_SESSION_READY=false and does not install a display-manager entry for the Rust compositor. It is a safe, *wired* backend improvement, not a claim that the entire compositor is finished.
