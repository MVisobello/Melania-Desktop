# Stage58 — Damage tracking on retained Wayland windows

DynamicWindowScene now records bounded output-clipped damage on client buffer commit, resize/shrink, move, removal, raise and output resolution change. The old AND new extents are invalidated so stale pixels cannot survive a move or shrink. Tests cover move, stale generation, focus and mode changes.

A damage region is a CPU-side hint, **not** DRM presentation. Only a verified real scanout ACK of the current exact scene generation may retire its regions via `confirm_presented_damage`. A newer scene retains damage. No KMS API calls, no native login; `NATIVE_SESSION_READY=false`.
