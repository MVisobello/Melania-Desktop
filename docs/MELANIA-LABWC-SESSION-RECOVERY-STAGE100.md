# Melania Desktop — resilient labwc session (stage 100)

## Delivery objective

A desktop cannot be a credible XFCE/Cinnamon replacement if a single taskbar or desktop-surface crash logs the user out and interrupts all applications. Stage 100 replaces the former Bash kill-check supervisor with a **Rust** supervisor for Melania's independently launched labwc-backed Wayland session.

This is the shortest practical route to an everyday Melania shell while the experimental DRM/KMS Smithay backend is made production-ready separately.

## Behavior

- The tiny `scripts/melania-labwc-supervisor` wrapper handles only session-local D-Bus portal environment updates and then **execs** `melania-session-supervisor`.
- The Rust supervisor validates a private user-owned XDG runtime and the exact user-owned `labwc` PID. It cannot manage unrelated XFCE/Cinnamon processes.
- It starts the private Wayland window-control adapter, then Melania Desktop Surface, Panel and notifications. It checks the adapter's actual Unix socket before starting the GTK shell.
- A single unexpected exit of **Panel**, **Desktop Surface** or **window-control adapter** schedules a restart after 650 ms, without terminating the compositor or already running user applications. A maximum of three restarts within sixty seconds prevents endless crash loops.
- Notifications may restart, but after repeated failures are disabled with an explicit diagnostic rather than tearing down all graphical apps.
- The secure `swayidle`/PAM-backed `swaylock` guard is mandatory **only in a full login**. If it exits, recovery is forbidden and the session logs out instead of continuing without protection.
- User XDG autostart launches exactly **once** per standalone login; restarts of shell components never relaunch startup apps.
- A nested preview skips host lock, suspend and user XDG autostart. It keeps its original host desktop and uses the exact same Rust shell recovery path.
- On shutdown, only the supervisor's own child process groups are terminated/reaped. On an unrecoverable failure the supervisor sends TERM only to the previously validated same-user labwc PID, **never to unrelated host processes**.
- The same private IPC and existing Melania panel interface continue unchanged.

## Acceptance

The Linux CI must compile and test `melania-session-supervisor`, run real labwc+GTK nested Wayland smoke inside disposable Xvfb, deliberately terminate the **exact Melania panel PID**, verify the supervisor's **recovery marker**, verify the compositor and IPC socket survive, and finally verify clean session QUIT.

Rust unit tests cover restart budget, exhaustion, guard isolation and symlink-safe IPC cleanup. Repository tests enforce session lock protection and singleton XDG autostart.

## Status / honest limits

This makes the tested **labwc-backed** Melania session more robust. It is not a migration of the native Rust compositor into production. The latter still requires resource reacquisition after VT pause, robust output hotplug, Xwayland, integrated security protocols and hardware/display-manager testing. Neither native readiness nor bare-metal login readiness is claimed by this stage.

### Try on a spare account before switching login manager

From the repository, build the relevant Rust workspaces and run `bash scripts/melania-labwc-session --check`, then stage with `--stage`. Test `--nested-test` first. The login-manager entry is **only staged** in the checkout; installation into `/usr/share/wayland-sessions` is a separate, explicit administrator step. Keep an existing XFCE/Cinnamon option for rollback.
