# Melania Native — Stage 102: seat-scoped DRM device selection

This increment builds on the real Smithay DRM udev event source added in PR #250. Its focus is **hardware ownership consistency**, not a claim of production readiness.

## Actual runtime change

Before the one-process QEMU Wayland/KMS experiment acquires its DRM descriptor, Melania now asks the `NativeGpuWatch` initialized from the **same libseat seat** which primary DRM card udev discovered. The selected `/dev/dri/cardN` path is passed directly to `session.open(...)`. No `/dev/dri/card0` guess remains in the integrated bridge.

- A single primary DRM card is accepted even when its number is not zero; optional render-only nodes are ignored for KMS selection.
- An empty, revoked, ambiguous (multiple primary DRM cards), or malformed device inventory aborts **before opening** the KMS descriptor.
- The verified libseat FD is still cloned into the internal Rust presenter; all existing KMS CRTC rollback, worker join, callback-epoch and libseat-close protections are retained.
- This does **not** pick an arbitrary GPU from a multi-GPU system. Multi-GPU routing requires a compositor-owned output manager and per-GPU DRM render surface, so this isolated VM experiment correctly rejects it.

## Test coverage

`cargo test -p melania-native --all-targets` exercises one non-card0 DRM device, render-only nodes, missing/ambiguous cards, malformed paths and irreversible revocation. Source integration tests verify the bridge uses the selected seat device rather than hardcoding a DRM node. Existing QEMU guest tests must still confirm real CRTC framebuffer ACKs and restoration.

## Architectural reference

The separation of seat-owned DRM device discovery from per-output scanout is informed by [COSMIC compositor](https://github.com/pop-os/cosmic-comp) and Smithay's public backend APIs. COSMIC is GPL-3.0-only; **no COSMIC code is imported** into Melania's MIT code.

## Remaining work before native Rust compositor can replace labwc

1. Persistent, event-driven per-output DRM/GBM/atomic renderer; not the fixed-duration QEMU proof.
2. Real multi-CRTC GPU/output routing and hotplug, scaling, and dynamic Wayland outputs.
3. Controlled VT suspend/reacquisition, scanout/GBM lifetime management, DRM master and verified hardware rollback.
4. Xwayland, input/IME, clipboard, portal, secure lock/login, and physical multi-GPU tests.

`NATIVE_SESSION_READY=false` remains mandatory. Use Melania's labwc-backed login session for real use; do not install this native experiment as a login compositor.
