# Stage57 — bounded native dirty regions

Stage53–56 established two independently updated, resized, moved real
Wayland client windows on virtual QEMU DRM/KMS, with framebuffer
verification. Those experiments currently composite and transfer a full
1280×800 BGRA image on each frame. This stage introduces a **pure Rust
damage-region planner** as a prerequisite to reducing CPU and GPU work.

\`DamageRegions\` computes output-clipped rectangles for window changes,
including signed/partially offscreen positions, content updates, move
(old **and** new bounds), resize/shrink, z-order changes and full output
resizes. Overlapping or touching rectangles merge; more than 64 disjoint
regions safely degrade to full-output damage. Geometry uses checked or
saturating 64-bit calculations; invalid output dimensions are rejected
before mutating the planner. Tests cover overflow, negative coordinates,
stale pixels after shrinking, union, disjoint rectangles, region budget,
and output resize.

**Important:** this PR does **not** activate partial GPU writes, change
the existing Smithay Wayland bridge, or treat a dirty rectangle as a
presented frame. The previous full-frame KMS pipeline and its kernel
FB_ID acknowledgements remain authoritative. A future stage will couple
damage snapshots to the existing generation ledger and only retire
regions after verified KMS ACK. Real hardware multi-output, VT lifecycle,
and login integration remain incomplete; NATIVE_SESSION_READY=false.
