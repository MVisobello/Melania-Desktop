# Stage109 — real Wayland scene rendered on GPU-backed GBM/KMS

This milestone addresses the main distinction between our earlier QEMU
CPU/dumb-buffer compositor proof and a hardware-backed Wayland compositor.
The implementation is **original Rust/Smithay code**, architecturally
informed by COSMIC's separation between output scenes, GPU renderer,
DRM pageflip events and seat ownership. No COSMIC GPL source was copied.

## Actual pixel path

1. The existing Smithay Wayland server receives real client `wl_shm`
   buffers and composes a 1280×800 owned image from client surfaces.
2. Its existing async frame worker sends the actual versioned `MLF1`
   image with epoch, sequence and scene generation to the integrated KMS
   presenter inside the **same** `melania-native` executable.
3. With the new QEMU-only GPU flag, the presenter validates the scene
   sequence and uploads that exact pixel array via Smithay `ImportMem`
   into an OpenGL ES **XRGB8888 texture**.
4. GLES draws the texture into the non-visible of two **retained actual
   GBM DMA-BUF scanout buffers**, imported as DRM framebuffer IDs.
5. The kernel displays the first frame via CRTC modeset; each later frame
   requires an actual DRM page-flip event, per-CRTC event-router receipt,
   monotonically sequenced scene identity and verified kernel FB_ID and
   connector/encoder/CRTC route.
6. Only after the kernel checks is an `MLA1` ACK sent to the Wayland
   bridge; only then are frame callbacks and client buffers released.
7. The original CRTC framebuffer and connector routing are restored
   **before** dropping any GBM buffers or releasing the libseat device.

An isolated QEMU integration test drives 24 real client commits through
this exact pipeline. The old CPU compositor and stand-alone GLES probes
remain separately tested; native login continues disabled.

## Known limitations

This first end-to-end GPU-backed path is finite (24 frames) and only on
one 1280×800 VirtIO output. The scene is currently composed into owned
CPU pixels before uploading as a texture; it is not yet zero-copy, direct
scanout, GPU-rendered per-surface composition, multi-output, production
atomic KMS, or VT-resume safe.

The next steps are to extend the GPU path to indefinite damage-driven
rendering (with signal shutdown), manage per-output GBM swapchains,
import dmabuf client surfaces with accurate feedback, implement native
output reconfiguration and VT/DRM master reacquisition, and test on
physical GPUs. Keyboard, clipboard, security and Xwayland integration
also require completion before replacing labwc.

`NATIVE_SESSION_READY=false`; no display-manager session changes.
