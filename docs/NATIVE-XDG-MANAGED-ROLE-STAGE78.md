# Native Stage 78 — collegamento xdg_toplevel al compositore sperimentale

Stage 77 introduced an isolated window-state reducer. Stage 78 connects real
Wayland xdg_toplevel maximize and unmaximize requests to that reducer and
to the dynamic retained scene in the disposable QEMU/KMS Wayland bridge.

- Reserve a stable role slot when the toplevel is created, before wl_shm commit.
- Update compositor state on maximize/unmaximize and send an xdg configure
  with the Maximized flag and requested output/restore size.
- Preserve window position across client commits. Normal client commits and
  real pointer drags update the remembered restore rectangle.
- Apply role position changes to the dynamic scene and schedule a bounded
  redraw through the existing real KMS ACK path.
- Forget per-surface role state when the role is destroyed.
- Keep the nested desktop visuals, titlebar styling and default CI modes intact.

The integration is opt-in only for a disposable QEMU dynamic/continuous KMS
proof using MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM in addition to the
existing VM-only guards. The new flag is not yet included in an end-to-end
QEMU role/pixel handshake test. The client must still ACK the configure
and commit an appropriately sized buffer; moving existing pixels alone
does not make a maximized window fill the output.

Not addressed by Stage 78: taskbar minimize/restore, fullscreen requests,
dynamic output geometry, fully autonomous DRM/VT session, login manager
integration, secure lock, and hardware validation.

NATIVE_SESSION_READY=false remains mandatory. Do not install a display
manager login entry based on this experimental stage.

Validation: cargo test -p melania-native --all-targets and the GitHub
Linux/Wayland/QEMU workflows; a dedicated real xdg maximize/restore
Wayland-to-DRM pixel test remains needed.
