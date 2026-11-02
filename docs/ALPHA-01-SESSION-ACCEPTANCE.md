# Melania Alpha 0.1 — session integration acceptance gate

This is a **test plan**, not a claim that native login is supported.

## Baseline and non-regression
- Keep `scripts/melania-session --nested` working under an existing graphical host.
- Keep `--check-native` returning 78 until a real native compositor has passed the gates below.
- Do not install a `wayland-sessions` entry or modify the display manager before those gates pass.
- Preserve Linux build, Wayland smoke and desktop integration workflows.

## Gate A — nested session (automatable in CI)
- [ ] Compositor starts and publishes a usable Wayland socket.
- [ ] GTK4 client opens and receives a frame.
- [ ] Desktop surface, panel and Start menu appear together.
- [ ] Super toggles Start; typing focuses Start search.
- [ ] Files and Settings can launch, close and relaunch.
- [ ] IPC workspace switching and screenshot capture succeed.
- [ ] Graceful shutdown leaves no orphan processes.
- [ ] Logs and screenshots are retained as CI artifacts.

## Gate B — native compositor (dedicated QEMU VM only)
- [ ] libseat acquires/releases seat and handles VT pause/resume.
- [ ] DRM FD is obtained through the session and output routing is validated.
- [ ] GBM/EGL renderer creates scanout-compatible buffers.
- [ ] Atomic TEST_ONLY succeeds before any real modeset.
- [ ] First actual KMS page flip is visible and captured in VM.
- [ ] libinput pointer and keyboard events reach Wayland clients.
- [ ] Hotplug, VT switching, logout and crash recovery restore display state.
- [ ] Multi-output and XWayland are verified separately.

## Gate C — login integration (only after Gate B)
- [ ] Provide a packaged, explicit opt-in session entry.
- [ ] Launch and logout via a display manager in a disposable VM.
- [ ] Verify user session services, D-Bus, notifications and portal behavior.
- [ ] Reboot and recover safely from a failed Melania session.
- [ ] Confirm fallback login into the existing desktop.

## Definition of done
Alpha 0.1 may be called a **nested preview** after Gate A.
It must **not** be advertised as an independently installable native desktop until Gates B and C are tested.

## Safety
No hardware acquisition or modesetting on a user's production machine as part of CI. Do not equate a successful build, a TEST_ONLY commit, or a running nested compositor with a successful native login.
