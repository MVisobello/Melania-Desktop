# Native Stage 79 — xdg fullscreen role requests

On top of Stage 78, the QEMU Wayland bridge now handles actual
xdg_toplevel fullscreen_request and unfullscreen_request callbacks.
The native window model sets or clears the Fullscreen state, advertises
1280x800 or bounded restored geometry in pending xdg configure and
updates the retained scene location for the next real client buffer.
Maximize also clears the Fullscreen flag, preventing contradictory role
states. The virtual output remains exactly one QEMU monitor; output hints
from clients are not interpreted as real multi-monitor DRM routing.

Rust regression tests cover maximize -> fullscreen -> unfullscreen and
preservation of client restore geometry. No titlebar or other shell visuals
are changed. This feature is gated behind the same disposable QEMU-only
MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM opt-in as Stage 78.

The change needs a dedicated client configure/ACK/KMS pixel end-to-end
test before being considered experimentally verified. The default native
login remains blocked (NATIVE_SESSION_READY=false).
