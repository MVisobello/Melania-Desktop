# Melania Native — Stage 105: signal-terminated persistent DRM/KMS experiment

This increment removes the **fixed number of presented frames** and the
**fixed-duration Wayland test loop** only in a new opt-in, isolated QEMU
one-process mode. The existing 12-, 14- and 24-frame QEMU acceptance scenarios
remain deliberately unchanged.

## Actual runtime implementation

The Smithay/libseat Wayland event loop can now continue dispatching after
the 24th frame for as long as it receives real client scene damage. Rather
than a synthetic timer-driven raster loop, frames are damage-driven and
back-pressured to one in-flight KMS submission; idle periods do not trigger
continuous busy-painting.

The in-process Rust DRM presenter retains the VM's two scanout framebuffers
and selected CRTC, listens for complete `MLF1` frame messages, requires
kernel page-flip/vblank and framebuffer/connector verification for every
asynchronous presentation, and sends `MLA1` only after the real kernel
checks. In persistent mode it accepts a clean EOF **only at a complete
frame boundary**. It then rolls back the original connector/CRTC and releases
its owned kernel framebuffer resources. EOF mid-header/pixel frame is fatal. Clean EOF with zero frames is
also safe: no modeset took place and resource restoration is a no-op.

The Wayland loop registers safe SIGINT/SIGTERM handlers (signal-hook) instead
of an unconditional test deadline. On a signal it:
1. stops enqueuing new frames;
2. drains an already-submitted DRM flip within four seconds, rejecting a
   stalled/failed flip;
3. revokes presentation permits;
4. stops and joins the sender, causing the receiver to observe EOF;
5. joins the DRM presenter (which attempts **verified CRTC restoration**);
6. closes the original DRM device through libseat only **after** the join.

The error path already uses the same conservative worker/join/rollback/close
sequence, and no panic or timeout is reported as successful shutdown.

## Real QEMU regression

`scripts/melania-qemu-native-guest.sh` now starts the existing one-process
server with `MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU` and **no**
sender/receiver frame target. It drives the real client through 24 verified
Wayland/DRM kernel frames, checks the compositor is **still running** after
the 24th, delivers SIGTERM, and requires real connector/CRTC rollback,
kernel event receipts and libseat release before claiming success.

## Limitations and safety

The new path is **not installable as a native login compositor**:
- It only activates inside a disposable QEMU VM with explicit consent,
  dedicated Linux VT and existing DRM/libseat checks.
- The hardware path still uses the QEMU output's two fixed 1280×800
  dumb buffers; these are not a production multi-monitor, per-output GBM
  swapchain with atomic modesetting.
- It does not yet recover GPU ownership after VT pause, manage Xwayland/IME,
  reconfigure real monitor modes, or establish a secure display-manager
  session. Those require dedicated implementation and physical hardware QA.

The current practical Melania session still uses labwc. Leave
`NATIVE_SESSION_READY=false`. No COSMIC GPL source is imported; Melania
uses Smithay public APIs and original MIT-licensed Rust.
