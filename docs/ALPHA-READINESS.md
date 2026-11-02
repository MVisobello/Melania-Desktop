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

## Release decision
Only label the build **Alpha** when all automated gates are green on the *same candidate commit*, manual gates have recorded results, and critical blockers are resolved. Otherwise label it **development preview**.
