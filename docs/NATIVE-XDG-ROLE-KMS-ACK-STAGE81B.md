# Native Stage 81B — real xdg roles, committed pixels, and KMS ACK

This disposable QEMU guest test extends Stage 81A to exercise actual
`xdg_toplevel` requests on a live Wayland client **with buffers**.

The first of two live xdg windows requests:
1. Maximize before real frame 3; attach an ACKed 1280×800 wl_shm buffer.
2. Restore before frame 5; attach a 360×224 buffer.
3. Fullscreen before frame 7; attach an ACKed 1280×800 buffer.
4. Exit fullscreen before frame 9; restore the 360×224 buffer.

The second window remains live on alternating frames. For all 12 frames
the test requires a real Wayland frame callback/buffer release after a
Smithay continuous-scene ACK and a Rust DRM/KMS worker acknowledgment
verified against the VM CRTC FB_ID. The complete VM script fails if any
role request, role-specific geometry, client callback, CRTC ACK or original
CRTC restore is missing.

This phase does NOT yet assert role-specific QMP screenshot pixel regions.
That belongs to Stage 81C. It also does not enable login, real-machine DRM,
VT session installation, lockscreen, polkit or production compositor mode.
`NATIVE_SESSION_READY=false`.
