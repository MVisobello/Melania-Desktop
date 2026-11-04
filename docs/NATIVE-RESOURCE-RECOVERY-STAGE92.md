# Stage 92 — native DRM/GBM/VT recovery contract and strict receiver budgets

This change addresses two failure modes while retaining the existing **QEMU-only** native test. It does not switch the production compositor from Winit to DRM/KMS.

## Resource recovery policy (Rust)

`melania-native/src/native_resource_recovery.rs` introduces a single-owner policy for:
- libseat activity, owned DRM master, GBM renderer, original scanout references,
  selected output, VT recovery capability, input enumeration and verified rollback;
- monotonic epochs: pause, re-acquisition and failure revoke outstanding frame receipts;
- one in-flight frame at a time, with no fixed frame-count ceiling in the *policy*;
- no rendering after VT pause; resume requires fresh evidence for **all** resources;
- a failed flip permanently poisons the current recovery object.

The individual milestones must be backed by **actual runtime kernel/libseat evidence** when integrating the production backend. Calling `confirm` by itself does not prove a resource exists. In particular, **the existing read-only atomic property inventory is not an owned CRTC baseline**. This module does not allocate GBM buffers, obtain DRM master, perform atomic rollback or install a login session. `native_login_authorized()` remains false.

`cargo test -p melania-native --all-targets` exercises >64 sequential frames, double ACK prevention, missed milestones, stale worker receipts, VT handoff and failed-flip recovery.

## Fix: malformed DRM receiver frame budgets

Previously `qemu-rust-kms-continuous` used `filter(...).unwrap_or(12)` even when an explicit count was malformed or incompatible with the other QEMU components. The receiver could wait for twelve frames while the sender expected another count.

The external receiver now calls the tested `native_frame_budget::receiver_frame_budget` parser:
- no explicit count: 12 only in ordinary QEMU mode;
- explicit ordinary count: 8–64 (legacy 8–11 preserved);
- real GTK image probes: require an explicit **exact** count of 1, 2 or 3;
- invalid/mismatched input: error **before** touching DRM/KMS.

The bridge/client Stage91 parser remains 12–64 for plain extended QEMU proofs.

## Validation and blockers

The QEMU workflow runs Rust unit tests and static regression tests. Its Linux/VM jobs must complete successfully on the final commit SHA before any integration. Because existing #220–#222 are stacked, this PR targets #222, not `main`.

**Still missing for a real session:**
1. Connect the resource policy to an actual long-running Smithay DRM/GBM renderer, allocate/retain scanout buffers, and remove the proof's fixed-number termination.
2. Preserve and atomically restore the real original framebuffer, kernel mode blobs, output state, DRM master and VT across stop, crash and seat pause.
3. Validate hardware recovery, multi-output hotplug and libinput; then integrate login manager, D-Bus, polkit and secure lock/logout in a disposable VM.

`NATIVE_SESSION_READY=false`. Do not install this branch as a system Wayland session.
