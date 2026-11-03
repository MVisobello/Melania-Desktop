# Stage54 — real xdg-shell resize configure/ack on the native QEMU KMS path

Stages50–53 introduced per-window GPU frame receipts, generic bounded
wl_shm import, owned window geometry and actual GPU/QMP proof that two
Wayland xdg_toplevel surfaces can shrink without leaving stale pixels.
Stage53's clients had chosen smaller buffers on their own, which is not a
compositor-managed resize.

The new **Stage54 disposable-QEMU-only opt-in** requires the real Smithay
xdg-shell configure/ack exchange. After the actual sixth DRM CRTC ACK,
the compositor sends new \`xdg_toplevel.configure\` dimensions using
\`with_pending_state\` and \`send_pending_configure\`:

- A receives 320×192 instead of its previous 400×240 pixels.
- B receives 220×180 instead of its previous 280×200 pixels.

Each real Wayland client records the toplevel dimensions and acknowledges
the corresponding \`xdg_surface.configure\` serial. It must not attach the
smaller wl_shm buffer until that ACK. Client A applies its new size at
frame7 and client B at frame8. Both must complete all 12 verified KMS
frame callbacks and buffer releases. The host QMP snapshot at frame12
still checks the **true** GPU display colors, premultiplied ARGB source
and exact right/bottom window edges and CRTC restoration.

The opt-in flags are \`MELANIA_NATIVE_XDG_RESIZE=YES_QEMU_VM\` on the
server and \`MELANIA_QEMU_XDG_RESIZE_CLIENT=YES_QEMU_VM\` on the client.
Invalid combinations are rejected. Existing fixed-size, close/reopen,
normal GTK desktop and nested compositor scenarios are unchanged.

**Not yet production resize:** libinput pointer-grab validation and the
full interactive request/drag/end protocol, arbitrary window roles,
multi-output GBM/DRM, VT pause/resume and GPU hotplug rollback, Xwayland
and display manager session boot. \`NATIVE_SESSION_READY=false\`.
