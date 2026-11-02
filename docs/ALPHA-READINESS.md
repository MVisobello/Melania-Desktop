# Melania Desktop — Alpha readiness gate

This checklist distinguishes **automated evidence** from **manual validation**. A green CI run is not a release certification.

## Automated gates (required)
- [ ] Linux build succeeds on the candidate commit.
- [ ] Wayland smoke succeeds with actual runner assignment and executed steps.
- [ ] Full nested desktop integration succeeds: compositor, desktop surface, panel, Files, Settings, input, screenshot and clean exit.
- [ ] QEMU DRM/KMS native smoke succeeds.
- [ ] Preserve screenshots and logs as downloadable workflow artifacts.
- [ ] Record commit SHA and workflow run URLs in the release notes.

## Manual gates (required before calling the build an installable Alpha)
- [ ] Install in a clean supported Linux VM without overwriting the host desktop.
- [ ] Log in to Melania session and log out; reboot and recover safely.
- [ ] Test Start keyboard activation, typing into search, power menu and focus.
- [ ] Test panel layout at top/bottom, tray toggles and clock alignment at different scales.
- [ ] Test Files: tabs, copy/move, rename, delete, drag-and-drop, GVfs mounts and permissions.
- [ ] Test Settings navigation: every page accessible, changes persist, XFCE-equivalent options audited.
- [ ] Test Light, Dark and Aero themes, icon coverage and window controls.
- [ ] Check multi-monitor, HiDPI, keyboard layouts and failure recovery.
- [ ] Record known limitations and provide an uninstall/rollback procedure.

## Preview install and rollback (per-user only)

The current stage is a **nested development preview**, not a standalone login session.
From the repository checkout on a Linux machine with an existing desktop:

```bash
bash scripts/melania-session --nested
# Optional: register/remove an application menu entry for the current user
bash scripts/install-melania-preview-launcher.sh
bash scripts/uninstall-melania-preview-launcher.sh
```

The installer touches only `~/.local/bin/melania-session` and a user application
entry under `${XDG_DATA_HOME:-$HOME/.local/share}/applications`. The rollback
checks the launcher and desktop entry before removing either one; it refuses
symlinks or unexpected/tampered content. Neither operation creates a
`wayland-sessions` or `xsessions` login entry.

## Release decision

**Nested Alpha preview:** automated gates green on the *same candidate commit*,
a documented clean Linux VM usability review, and all critical nested blockers
resolved. This mode runs within an existing graphical host.

**Standalone installable native Alpha:** additionally requires the hardware
DRM/KMS + libseat/Wayland input/lifecycle and display-manager login, logout,
recovery gates in `docs/ALPHA-01-SESSION-ACCEPTANCE.md`. A QEMU
`TEST_ONLY` pass is not proof of a native graphical session.

If any gate is incomplete, keep the label **development preview**. Record the
commit SHA, successful workflow links, machine and manual observations, and
known limitations instead of inferring readiness from a runner starting.
