# Continuous native redraw — planner milestone (experimental)

**Status:** The Rust `ContinuousRedraw` planner and unit tests are implemented. It is **not** wired into the real DRM/KMS scanout loop, and this is **not** a login-ready desktop session.

## Why separate this work from QEMU Stage 40/41?

The disposable QEMU scripts use up to six deliberately numbered presentations to prove that individual `wl_shm` buffers were actually scanned out. This is useful for validating close/reopen and restoring the prior CRTC, but a usable compositor must not stop presenting after frame six.

`melania-native/src/continuous_redraw.rs` uses the existing `RenderScheduler` ticket and epoch rules without any new threads, framebuffer allocations or DRM device accesses.

- **Scene damage:** changes to `MultiWindowScene::layout_generation()` create one dirty event. Reobserving the same generation is a no-op.
- **One frame in flight:** `next_frame()` returns a `PlannedSceneFrame { ticket, generation }` only when the seat is active and there is no outstanding presentation.
- **Backpressure:** 100 scene mutations while KMS is presenting collapse to the **latest** scene generation and one successor ticket. There is no unbounded queue.
- **ACK discipline:** `presented(ticket)` must only be called after the kernel display operation has been confirmed. A wrong, stale or duplicate ticket is never accepted as proof of presenting.
- **Failure and lifecycle:** `failed(ticket)` requests a retry; seat pause, reactivation and output hotplug invalidate old epochs and force a new redraw of the current scene.
- **No busy-loop:** with no changes and no pending ticket, there is nothing to render.

## What must still be integrated

1. In the native compositor's calloop event loop, send every real scene damage event to `scene_changed(scene.layout_generation())`.
2. When a frame becomes ready, call `next_frame()`, composite the associated *current scene*, and pass the ticket, its generation and pixels to the bounded asynchronous KMS worker. Do not perform KMS I/O on the Wayland dispatch thread.
3. Pair a result with exactly the ticket and generation submitted. On confirmed FB_ID/page-flip completion, call `presented(frame)`; only then send that surface's `wl_surface.frame.done` and `wl_buffer.release`.
4. Ensure output mode changes, VT switch, session pause/resume, device removal and GPU reset revoke pending work before releasing buffers.
5. Replace the fixed-ordinal QEMU transport only in a **separate**, explicitly opted-in virtual-machine proof, with continuous paint/close/reopen cycles, real QMP screendump pixel checks, bounded memory, independent failure tests and guaranteed restoration of the original CRTC.
6. Only after stable hardware and VM tests should native-session packaging/login be considered.

The existing nested preview and desktop components are unaffected. The flag `NATIVE_SESSION_READY=false` remains authoritative.
