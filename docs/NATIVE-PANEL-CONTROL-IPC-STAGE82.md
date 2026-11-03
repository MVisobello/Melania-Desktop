# Stage 82 — Melania Panel IPC activates and restores real Wayland windows

After Stage 81, the disposable QEMU native Wayland bridge reuses the
existing `melania-control` crate and Melania Panel's LIST, ACTIVATE,
MINIMIZE, CLOSE, STATUS framing. The exact same command syntax can be
used by the current Melania Panel without a parallel ad-hoc control API.

The compositor creates a **0600 Unix socket** in private
`XDG_RUNTIME_DIR` (directory permissions must deny group/other access),
and removes the socket when the VM probe exits. The endpoint is enabled
only with `MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM` after QEMU/native-VT
preflight. It is not a production session or a login manager endpoint.

- LIST returns real xdg window titles/app IDs and active/minimized flags.
- ACTIVATE restores a minimized Stage77 model, unhides retained
  Stage81 client pixels, raises the window, configures Activated state,
  transfers keyboard focus and queues frame damage through the real
  KMS ACK path when a scene exists.
- MINIMIZE and CLOSE are dispatched to existing Smithay role callbacks.
- STATUS returns workspace 1; workspace switching and PREVIEW are
  unsupported until the native backend can provide those capabilities.
- QEMU's independent shell client minimizes a real Wayland toplevel,
  uses LIST to confirm minimized, sends ACTIVATE 1, and uses LIST again
  to confirm it is active and unminimized. The guest fails if this
  roundtrip or the server-side activation marker is missing.

**Not yet proved by this protocol smoke:** the role's new pixels
reaching the real QEMU CRTC. This remains required before a native
desktop session can be installed safely.

`NATIVE_SESSION_READY=false` remains mandatory.
