# Stage 51 — generic owned wl_shm client pixel import

The original native QEMU client probe only accepts XRGB8888 buffers of
360×224 pixels. That narrow demo is now backed by a reusable Rust importer
(owned_shm) that accepts *bounded, arbitrary* surface sizes and copied
client pixels, preparing the native backend for real GTK windows.

The importer reads data only inside the live Smithay shm-access callback.
It validates signed geometry, pool offset, stride, dimensions and
checked arithmetic BEFORE allocating, caps copied data at 64 MiB and
rejects null, truncated, oversized and unsupported mappings. It removes
stride padding into tightly packed BGRA owned by Melania, never preserving
a borrowed client pointer past the callback.

Both Wayland XRGB8888 and ARGB8888 formats are supported. ARGB8888 data
is marked as PREMULTIPLIED alpha; clients' alpha bytes are preserved.
It must not be treated as straight alpha in the future scene renderer.

To ensure regression coverage, the existing fixed-size QEMU import now
uses this generic function while deliberately retaining its dimensions
and XRGB-only protocol restriction. Thus existing QMP pixel comparisons
exercise the same importer as the future general-purpose scene.

What it does NOT do: resize the current two fixed QEMU window slots,
render arbitrary GTK surfaces with alpha, implement dynamic monitor
modes or change an actual DRM output. These require a dynamic window
tree and output renderer. Native login stays disabled and the approved
Melania GUI remains unchanged.
