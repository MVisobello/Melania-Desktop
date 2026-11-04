# Stage 94 — coordinated native output dispatch ledger

The native session has independent safety state machines for resource
acquisition/restoration (Stage92), retained-scene redraw and frame deadlines.
Stage94 places them behind one output dispatch contract, rather than
allowing a future renderer to bypass one of them.

## What is implemented

`NativeOutputDispatch` in
`melania-native/src/native_output_dispatch.rs`:

1. Blocks dispatch until the caller has confirmed **all eight resource
   milestones** for the current acquisition or restoration epoch.
2. Coalesces scene changes and grants at most **one** outstanding submission.
   There is no QEMU ordinal or hardcoded six/twelve/64-frame ceiling.
3. Arms a bounded `KmsFrameWatchdog` before exposing the submission.
4. Accepts a completed presentation only after the caller says the **actual
   kernel CRTC** was verified, the timeout has not elapsed, and both scene and
   resource tickets are still current. Only a `true` return authorizes the
   outer backend to send Wayland frame callbacks or release buffers.
5. Revokes all pending submissions on libseat pause. Reactivation requires
   fully rebuilding all eight resource proofs.
6. Fails closed after a rejected flip, timeout, device removal or uncertain
   output restoration. A failed resource generation is never re-armed.

Unit tests exercise 256 frames, coalescing, duplicate and stale ACKs,
VT pause/restoration, timeout boundaries, and permanent GPU failure.

## What is **not** implemented

This remains a pure Rust coordination layer, **not** an operational DRM
renderer. It does not obtain session FDs from libseat, create a GBM/EGL
renderer, own original framebuffer storage, set an atomic CRTC, restore
the console VT, or handle crashes of the actual display-server process.

The `kernel_presented` argument is supplied by the future backend. Its
truthfulness **must** come from verified kernel presentation, never from
an unverified socket ACK, timer or synthetic test event. The existing QEMU
receiver and the live Smithay bridge remain separate.

Do not register Melania as a default display-manager session on the basis
of these tests. `NATIVE_SESSION_READY=false`.
