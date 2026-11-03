# Stage 46 — QEMU-only continuous compositor frame loop

**Implementation submitted for real QEMU verification.** This is still NOT
a production native-session renderer or a complete desktop.

The Smithay experimental bridge now has a separate opt-in flag
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM. In that guarded mode it:

- imports each real client-owned wl_shm buffer into retained MultiWindowScene;
- records actual scene generation changes in ContinuousRedraw;
- submits the latest composed scene to a one-job bounded KMS socket worker;
- matches epoch, sequence and generation against the DRM receiver ACK;
- accepts the planner ticket only after confirmed CRTC FB_ID presentation;
- then delivers the Wayland frame callback and wl_buffer.release;
- processes each next dirty scene without the old 1..=6 ordinal ceiling.

The new qemu-continuous-client issues twelve different real wl_shm commits
on an xdg_toplevel and waits for a frame.done plus buffer release for each.
The independent qemu-rust-kms-continuous receiver uses two preallocated dumb
buffers, checks twelve actual CRTC states and restores the original CRTC.
The guest shell script and host CI gate fail if any frame or restoration
acknowledgement is missing.

## Scope, limitations and next requirements

The twelve-frame target is an explicit disposable guest exit criterion.
The protocol itself uses 64-bit ticket sequences and has no sixth-frame
limitation. More than six real frames are only *proven* after the QEMU Actions
job completes successfully.

There is still more work before native login or general-purpose continuous
redraw: two independently animating windows, destruction/reopen and drag while
KMS is busy, handling stale ACKs on VT switch/hotplug, production-grade
buffer ownership and per-surface callbacks, real hardware verification,
X11/Xwayland and a graphical session manager. These cannot be claimed from
this VM experiment.

No changes to Melania panels, files, themes, icons, controls or layout.
NATIVE_SESSION_READY=false remains mandatory.
