# Stage49 — live continuous Wayland window close and reopen

Stage48 validated two retained xdg_toplevel windows, each independently
submitting wl_shm buffers through a generation-based DRM/KMS scheduler.
Stage49 proves that the same QEMU-only pipeline can subsequently:

1. Reach twelve real presentations, leaving two window snapshots visible.
2. Destroy A while B remains alive. B requests a real wl_surface.frame.
3. Schedule compositor-only frame 13 on A's removal (no extra client pixel
   commit), verify FB_ID scanout, and only then send B's frame.done.
4. Create a DIFFERENT xdg_toplevel and commit its new wl_shm buffer.
   The compositor reuses free slot 1, composes A with B and presents frame 14.
5. Verify frame 14's callback and buffer release only after real KMS ACK.
6. Take actual QEMU QMP screenshots during frame 13 (A blank/B retained)
   and frame 14 (new A/B retained), then restore the original CRTC.

This scenario is separately opted into by the disposable QEMU-only
MELANIA_NATIVE_CONTINUOUS_REOPEN and MELANIA_QEMU_CONTINUOUS_REOPEN flags.
It does not change the tested twelve-frame single-client or two-client
scenarios, and it does not remove the safety stop at the VM test target.

The experimental proof is NOT a production Wayland compositor. Still
required: normal arbitrary window sizes and counts, robust buffer
lifetimes and frame callbacks on damage coalescing, VT pause/resume and
output changes, GBM/DRM hardware integration, Xwayland, login manager and
session services. Native session readiness must remain false; Melania
themes, icons, taskbar, buttons and menu remain untouched.
