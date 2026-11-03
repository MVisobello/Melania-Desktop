# Stage55 — real pointer focus and move use variable Wayland geometry

Earlier VM experiments proved KMS scanout for multiple separately sized
xdg_toplevel surfaces. The real libinput pointer button path still used
the obsolete fixed 360×224 scene for hit-testing, focus and drag geometry,
so a rendered dynamic window was not actually clickable.

Stage55 fixes that input/render inconsistency without changing the
approved Melania user interface. The Smithay QEMU bridge now:

- Resolves pointer targets against DynamicWindowScene whenever the
  explicit dynamic QEMU mode is enabled.
- Raises the actually clicked retained client in z-order, then focuses
  that client's real wl_surface through Smithay keyboard focus.
- Starts move interactions using the **current** owned client width
  and height, not the old fixed 360×224 dimensions.
- Moves the retained window in the dynamic scene and schedules a
  new composed KMS frame only while the validated VM stream is active.
- Leaves bottom-right resize as a non-destructive preview unless a
  separate xdg-shell configure/ACK has approved the new pixel dimensions.
- Retains the existing fixed-window input and rendering behavior.

Rust unit tests confirm hit-testing, sizing and removal after a dynamic
window shrinks and moves. Python source-contract tests audit input
routing and ensure no direct mutation of unacknowledged client pixels.

This is NOT yet an end-to-end pointer-injected QMP proof of dynamic
dragging. That requires a follow-up host real libinput injection test
with a final framebuffer capture. It does not enable native login or
give the compositor control of an actual host monitor.
\`NATIVE_SESSION_READY=false\`.
