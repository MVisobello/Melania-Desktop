# Native Stage53: real variable-sized Wayland windows via continuous QEMU DRM/KMS

Stage53 is an **isolated VM-only opt-in** that connects the three previous
components: Stage50 per-client KMS receipts, Stage51 owned variable-size
wl_shm import, and Stage52 retained dynamic scene.

The existing QEMU scenario with two windows sized 360×224 and the
fourteen-frame close/reopen proof remain untouched.

New stage: a distinct disposable QEMU guest scenario launches TWO
xdg_toplevel surfaces. Real client A initially submits a **400×240**
wl_shm buffer and shrinks to **320×192** on frame 7. Real client B
initially submits a **280×200** buffer and shrinks to **220×180**
on frame 8. The clients continue alternating until frame 12.
The Smithay bridge imports and validates each buffer,
keeps a compositor-owned pixel snapshot, composes the full 1280×800 image
with correct window sizes, and submits it via the existing versioned
MLF1/MLA1 protocol. Each wl_surface.frame callback and wl_buffer.release
is allowed only after the virtual CRTC FB_ID has been verified.

The Rust receiver emits a live frame12 marker; a separate host-side QMP
screendump captures **actual GPU display pixels**. It verifies both
windows and the exact right and bottom edge of each: A at x160–479,
y120–311 and B at x650–869,y190–369. Pixels immediately outside these
rectangles must be background, not copied from the old 360×224 test.

Client B also uses real Wayland **ARGB8888** with premultiplied RGB at
half opacity. Its QMP pixels are checked against the composited value
`source_premultiplied + background * (1-alpha)`, which rejects applying
alpha twice. This validates the transparent client route end-to-end
through owned SHM import, the CPU scene and actual KMS scanout.

The test requires all 12 DRM ACKs, both client callbacks, the QMP pixels,
and exact CRTC restoration. No production login/session is enabled,
there is no host DRM access, and the Melania UI (menu, tray, Files,
window controls, themes and icons) is unchanged.

Still missing for daily use: true general-purpose xdg-shell resize
negotiation, arbitrary window count and positions, KMS/GBM multi-output,
VT pause/resume and GPU hotplug recovery, Xwayland and login integration.
NATIVE_SESSION_READY remains false; passing Stage53 is not a release.
