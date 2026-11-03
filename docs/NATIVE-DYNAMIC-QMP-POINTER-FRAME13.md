# Native Stage 56 — real QEMU tablet drag after negotiated Wayland resize

This is a **disposable QEMU VM experiment**, not a complete desktop compositor
or permission to start a native graphical login session.

## Proof required

1. Two independent real xdg_toplevel Wayland clients attach owned wl_shm pixels,
   ACK serial-bearing xdg_surface resize configures, and shrink to **A 320×192**
   and **B 220×180**.
2. The existing frame-12 real Rust DRM/KMS CRTC ACK occurs with client A and B
   retained, and the QMP host watcher verifies their old screen positions.
3. After the VM-only **unique** `MELANIA_NATIVE_DYNAMIC_POINTER_READY_AFTER_KMS12=true`
   server signal (not a generic ordinal-12 message shared by earlier stages),
   the *host* injects USB-tablet motion and a physical left button through QMP:
   start (760,200) inside B's resized top strip, move to (850,280), release.
   The Smithay/libinput bridge uses **current** B geometry, grants Wayland focus,
   moves B from roughly (650,190) to (740,270), and queues compositor damage.
4. A **thirteenth** real GPU KMS frame is submitted without another client
   wl_shm attachment. The Rust guest presenter verifies the actual CRTC FB_ID
   before its versioned MLF1/MLA1 ACK.
5. A second QMP screendump must prove B's old pixels were cleared, the 220×180
   premultiplied ARGB buffer appears in its new position, and A retains frame11
   pixels. Pixel checks use individual RGB coordinates and bounded tolerances.
6. The native bridge verifies B's position and dimensions **after** frame13 ACK.
   The client stays alive until an ACK-only VM marker appears, to avoid confusing
   client destruction with pointer-driven repaint. All runs restore the original
   virtual CRTC; no physical host GPU access is allowed.

## Gates and safety

All three explicit opt-ins are required for this new scenario:

- Server: `MELANIA_NATIVE_DYNAMIC_POINTER_PROOF=YES_QEMU_VM` along with continuous,
  two-window, dynamic-scene and negotiated xdg-resize QEMU gates
- Client: `MELANIA_QEMU_DYNAMIC_POINTER_CLIENT=YES_QEMU_VM`
- KMS guest receiver: `MELANIA_QEMU_CONTINUOUS_DYNAMIC_POINTER=YES_QEMU_VM`
  with `MELANIA_QEMU_CONTINUOUS_FRAMES=13`

Without these flags the older twelve-frame and fourteen-frame proofs keep
their original behavior. Exactly twelve wl_surface.frame callbacks are expected:
frame13 is a compositor-only redraw, not a new client commit.

The QEMU native GitHub Actions workflow must complete Linux Rust tests, SFTP
and virtual-CRTC screenshot validation before this stage may merge.

**Remaining:** a general-purpose production session, hotplug and VT switching,
non-VM libinput policy and multi-monitor management. `NATIVE_SESSION_READY=false`.
