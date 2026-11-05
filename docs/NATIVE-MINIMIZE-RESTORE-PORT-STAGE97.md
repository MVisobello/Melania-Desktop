# Stage 97 — native window minimize/restore ported to current layered KMS scene

## Why this change

Historical PRs #192 and #193 implemented preliminary minimization and a taskbar IPC roundtrip on pre-Stage96 branches. The current `main` retained scene also tracks Wayland layers and verifies DRM CRTC/connector routing. Blindly merging the old code would discard the newer layer behavior. This change ports the **behavior**, not the old branch state, onto current `main`.

## Implemented

- `DynamicWindowScene` retains the owned `wl_shm` image, memory accounting, stable ID, and stacking layer when a window is minimized.
- Hidden clients are omitted from full-frame composition, dirty-rectangle composition, and pointer hit testing.
- Hide and reveal invalidate the whole retained rectangle; subsequent hidden buffer commits remain invisible until explicit restore.
- The real QEMU `xdg_toplevel.set_minimized` request updates `ManagedWindow` and revokes keyboard/pointer focus from the hidden role.
- The VM-only `MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM` bridge exposes the existing `melania-control` framing on a mode-0600 Unix socket within a non-symlink, mode-private `XDG_RUNTIME_DIR`. The socket is removed when the process exits.
- LIST, ACTIVATE, MINIMIZE, CLOSE and STATUS operate on real Smithay toplevel roles. Unsupported commands return an explicit error; no generic shell execution.
- ACTIVATE restores retained pixels (if present), stacking, geometry, keyboard focus and client configure state. Scene changes are routed to the existing continuous KMS scheduler; buffer recycling remains subject to the existing verified KMS ACK rules.
- The QEMU shell probe requires independent server and real Wayland client markers for minimize, IPC LIST before and after ACTIVATE, and restored active/minimized state.
- Rust regression test covers preservation of buffer ownership across minimize, layer-aware hit testing, hidden updates, and full/partial redraw.

## Safety and limits

This is still a **QEMU-only experimental native backend**, not a production login session. The role smoke uses a client without painted content, so an IPC roundtrip is **not** proof that a restored role's pixels reached a KMS CRTC. Subsequent development must add a separately verified pixel/CRTC replay of the minimize/restore transition, release/re-acquire DRM/GBM resources across VT events, and test the display manager's login/logout and screen-lock lifecycle.

Do not register a production native session. `NATIVE_SESSION_READY=false` remains the correct status.
