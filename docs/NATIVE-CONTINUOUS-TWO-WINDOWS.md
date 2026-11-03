# Stage 48: two independent continuous Wayland windows in disposable QEMU

The stage-46 proof was one real xdg_toplevel generating twelve successive
wl_shm commits. Stage 48 adds a SECOND distinct xdg_toplevel and verifies
A1, B2, A3, B4 ... A11, B12 as twelve genuine Wayland commit → KMS ACK
→ frame callback/release cycles. Both windows retain their own copied
pixels in MultiWindowScene; the scene's generation ledger remains
authoritative and is not bounded by the old frame index 6.

The new mode is guarded by both
MELANIA_NATIVE_CONTINUOUS_KMS=YES_QEMU_VM and
MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM. The existing one-client
and legacy six-frame paths remain unchanged.

The guest additionally checks that both surfaces existed *at the verified
twelfth KMS ACK*, rather than merely when its delayed shutdown handler
executes. This handles clients that destroy their toplevels immediately
after receiving the final frame callback.

The real QEMU KMS recipient emits a distinct marker when dual-window
frame 12 is currently scanned out. The host QMP checker captures a PPM
and samples window A's retained frame 11, window B's fresh frame 12,
both borders, and the background. It fails if one window vanished or
the physical framebuffer failed to update. The CRTC must be restored.

Passing this experiment establishes TWO retained windows continuously
updating on an isolated virtual GPU. It does NOT yet provide a production
native desktop session, arbitrary windows, reliable focus/resize policy,
unbounded buffers, hotplug recovery, Xwayland or login integration.
Keep NATIVE_SESSION_READY=false and do not alter Melania's UI.
