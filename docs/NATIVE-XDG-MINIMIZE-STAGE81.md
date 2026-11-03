# Stage 81 — real xdg_toplevel minimize request to retained pixels

Connects the real Smithay Wayland `minimize_request` callback to the Stage77
`ManagedWindow` reducer and Stage53 dynamic retained scene. This is active
only with the existing disposable QEMU `MELANIA_NATIVE_MANAGED_XDG` flag.

- `DynamicWindowScene::set_visible` hides the window while retaining its
  compositor-owned wl_shm image, client slot, and bounded memory ownership.
- Hidden surfaces are omitted from both full and partial composition and
  pointer hit testing. Re-showing the surface invalidates its full rectangle.
- A minimize request triggers compositor damage for the next KMS frame,
  and immediately revokes keyboard and pointer focus from a hidden surface.
- The QEMU role-only client issues a real `xdg_toplevel.set_minimized`
  request. Guest CI requires the server to log receipt of that request.
- Rust regression tests check memory preservation, hidden buffer commits
  and correct input hit testing, then verify pixels reappear on restore.

**Still missing:** a taskbar or shortcut restore action wired into this
native QEMU bridge; xdg_toplevel has no client-side unminimize request.
Client configure-to-KMS pixel checks for role transitions are a separate
end-to-end milestone. No production native login or hardware takeover.
`NATIVE_SESSION_READY=false` remains the correct status.
