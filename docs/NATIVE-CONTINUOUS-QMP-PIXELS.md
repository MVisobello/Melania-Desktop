# Stage47: independent QMP pixel proof of frame 7 and frame 12

This is a stricter verification layer on the Stage46 QEMU-only end-to-end
continuous Wayland/KMS experiment. The Rust receiver now keeps actual
framebuffer #7 and #12 visible for 2.5 seconds, and a separate host-side
QMP client captures their PPM pixels while the receiver still owns the VM
CRTC. The script checks the virtual GPU resolution, image size, background,
window border and the different colors supplied by the real Wayland
wl_shm client for each numbered generation.

The host CI job fails if either QMP image, pixel comparison, KMS ACK or
original CRTC restoration is missing. The original six-frame QMP pixel
proof remains required and unchanged.

The temporary VM exit condition remains 12 frames, NOT a production
desktop frame limit. Native login, Xwayland, GPU hotplug recovery and
multi-client window management are still unresolved and disabled.
