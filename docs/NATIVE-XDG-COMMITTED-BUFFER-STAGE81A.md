# Native Stage 81A — xdg role configure → real client pixels (QEMU only)

This bridges an actual Wayland `xdg_toplevel` request to the existing
dynamic retained scene's **real wl_shm commit path**. Prior Stage 78/79
could move the old retained image and schedule a KMS redraw at the
new maximized/fullscreen position *before* the client supplied a resized buffer.

In opt-in QEMU `MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM` mode:
- A changed maximize, restore or fullscreen role records the expected width
  and height after issuing `xdg_toplevel.configure`, but **does not**
  re-present stale pixels merely because the role changed.
- Imported client `wl_shm` images are checked against that expected size.
  Stale/mismatched images are released and cannot mutate the retained scene.
- Only an accepted image updates the scene, marks damage and enters the
  existing continuous KMS scheduling/ACK pathway. The pending expectation
  is retired after successful insertion, and destroyed roles are cleaned up.
- Unmanaged QEMU tests retain their original handling and initial managed
  windows remain free to commit valid non-360x224 pixels before role changes.

This is **not** yet the full Stage 81 end-to-end role-specific QMP pixel proof:
the disposable QEMU client must still exercise each role transition with
correctly resized buffers and verify corresponding KMS CRTC ACK and QMP
pixel regions. Session bus, polkit, secure lock, hardware VT lifecycle and
production login remain outside this change.

`NATIVE_SESSION_READY=false`. No native login entry, host DRM acquisition
or visual/style changes are enabled.
