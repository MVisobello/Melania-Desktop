# Native focus-follows-pointer (QEMU opt-in)
This implements a COSMIC-style *sloppy focus* policy on the real Smithay
Wayland keyboard seat, driven by real libinput relative or absolute motion.
The default remains **click-to-focus** (no behavior change for existing users).

In a dedicated disposable QEMU guest, explicitly enable:
`MELANIA_NATIVE_FOCUS_FOLLOWS_POINTER=YES_QEMU_VM`,
`MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM`,
`MELANIA_NATIVE_DYNAMIC_SCENE=YES_QEMU_VM`, and
`MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM`, alongside the existing
required DRM/VT/KMS guest authorization. This is **not** a login mode.

The pointer targets the topmost visible, hit-tested owned Wayland XDG
toplevel (not an XDG popup). Only on actual hover changes does the compositor
send `wl_keyboard` focus to that surface. Hover never raises a window,
submits a fake KMS frame, or changes fullscreen/tiling geometry. It ignores
unmapped targets, held mouse buttons, compositor drags/resizes, grabbed popups,
already-focused windows, inactive seats and revoked presentation epochs.
Held buttons reset on device removal, libseat pause and udev GPU loss.

**Acceptance boundary:** Pure Rust policy unit tests and structural
input-wiring regressions are present. Full independent QEMU USB mouse motion
→ two real Wayland toplevels → observed keyboard-enter transitions, and
physical multi-monitor/VT testing remain to be done. Do not claim COSMIC
parity or enable `NATIVE_SESSION_READY` from this feature alone. Preserve labwc.
