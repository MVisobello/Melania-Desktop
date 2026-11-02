# Stage 38 — xdg-shell resize configure/ACK under real QEMU input

When the pointer grabs the bottom-right 16×16 px corner of a real
`xdg_toplevel` in Stage36/37's isolated QEMU session, the native Smithay
bridge may now issue **a real xdg_toplevel.configure** with a bounded
requested size. Its pending state includes `Resizing`; release clears that
state and sends the terminal configure.

The actual separate Wayland client subscribes to xdg_toplevel configure,
receives the requested non-default dimensions and explicitly calls
`xdg_surface.ack_configure(serial)` on the matching surface configure.
Both facts are observed in the guest serial log. Stage38's separate private
QMP injector moves the VM USB tablet to the window corner, presses and
drags before release, and refuses to pass if the client configure/ACK
markers are absent.

**Limit:** the resized client `wl_shm` buffer is not yet imported and
the retained scene still uses 360×224 buffers. Stage38 proves the
protocol request and ACK, **not a truly resized on-screen window**.
Full visual resizing requires dynamic-size imported buffers, surface
tree geometry, damage, and a subsequent independently verified KMS
frame, all scoped to VM tests before production use.

The QEMU-only controls, original CRTC restore, real frame pixel tests,
and `MELANIA_NATIVE_SESSION_READY=false` remain mandatory.
