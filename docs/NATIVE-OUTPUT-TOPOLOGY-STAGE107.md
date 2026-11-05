# Melania Native — Stage 107: kernel-backed per-output routing baseline

COSMIC Comp is used as an **architectural reference**, not a source-code
donor: its Rust/Smithay design separates GPU ownership, connectors, CRTCs,
per-output render surfaces and output reconfiguration. Melania implements
its own logic using the public drm-rs and Smithay APIs under its own license.

## Implemented

`melania-native/src/native_output_topology.rs` now reads an **actual live
DRM descriptor already owned by the session**:

- enumerate real DRM connectors and CRTCs (bounded to 32 each);
- inspect connected connectors, their active encoders and associated CRTCs;
- read actual original CRTC framebuffer IDs, modes and pixel dimensions;
- capture active outputs independently (up to eight) and reject a CRTC
  mistakenly shared between two connectors, duplicate connector, invalid
  sizes or incomplete framebuffer/mode;
- read the topology *twice* before rendering and reject changes between
  baseline reads; this reduces, but cannot eliminate, a hotplug race.

This observational topology is connected to **both real KMS paths**:
the embedded Smithay Wayland page-flip receiver and the GBM/EGL scanout
probe. Neither chooses an arbitrary first connected connector any more.
Both still deliberately require exactly one selected active output in their
isolated 1280×800 QEMU test, and both recheck the original framebuffer
after the baseline and each presented frame.

The QEMU guest now verifies the topology marker for the GBM and integrated
Wayland renderer. Rust unit tests cover two independent outputs, duplicate
or cloned CRTCs, capacity and invalid framebuffer sizes. Read-only
inventory does not acquire any ownership of framebuffer objects.

## Remaining work before native login

This change does **not** enable multiple monitors to scan out
simultaneously. A production backend still needs:

1. independent per-output GBM framebuffers/DRM surfaces and atomic state;
2. per-output Wayland render state and damage tracking;
3. libseat VT pause/resume and GPU hotplug re-acquisition with verified
   resource rollback;
4. output configuration, scaling/rotation, Xwayland and input integration;
5. long-running QEMU and physical-GPU testing.

Keep the existing stable labwc session and
`NATIVE_SESSION_READY=false`. No source is copied from cosmic-comp
(GPL-3.0-only).
