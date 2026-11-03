# Native Stage 80 — QEMU real Wayland role configure and ACK

The Stage 78/79 compositor callbacks now have a dedicated *real-client*
protocol smoke sequence, rather than relying only on isolated Rust
reducers. The disposable QEMU shell probe sets
`MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM` on both server and client.

The independent Wayland client requests, in order:
1. xdg_toplevel.set_maximized (configure size 1280x800, Maximized set);
2. unset_maximized (configure restored 360x224, Maximized clear);
3. set_fullscreen (configure 1280x800, Fullscreen set);
4. unset_fullscreen (configure restored 360x224, Fullscreen clear).

Every transition requires the client's real xdg_surface ACK, size and
compatible state flags. The guest script rejects missing client or
server markers, rather than reporting a successful VM boot as a pass.
An opt-in exception in the role integration guard permits this protocol
proof in the existing disposable QEMU `MELANIA_NATIVE_SHELL_PROBE` path;
the parent binary still requires explicit QEMU DMI, opt-in and Linux VT.

This is NOT a pixel or CRTC presentation test: the shell probe does not
submit client buffers to DRM. The earlier QEMU KMS tests remain enabled
and unchanged, and a role-specific maximize/restore pixel proof is still
a separate prerequisite for a true standalone session.

`NATIVE_SESSION_READY=false` remains correct. No host login changes.
