# Stage 101 — Melania at Linux login, reversible and per-user

## Goal

Make the already tested labwc-backed Melania environment selectable at the login manager while retaining Cinnamon/XFCE as fallbacks. The experimental native Rust DRM compositor is NOT installed as a production login session.

The system-wide entry is a root-owned generic launcher with no embedded user checkout path. Each account registers its own compiled Melania checkout in a private configuration file.

## Preparation on a spare Linux user account

1. Build the components in the checkout, without root privileges:

    cargo build --release -p melania-panel -p melania-desktop-surface -p melania-files -p melania-settings -p melania-services -p melania-notifyd

2. Install system dependencies (labwc, wlrctl, swaylock, swayidle, dbus-run-session, xdg-desktop-portal-wlr; Xwayland where needed). On Mint 22 the project may need its private GTK4 layer-shell library; never replace system GTK.
3. Run the preflight and stage:

    bash scripts/melania-labwc-session --check
    bash scripts/melania-labwc-session --stage

   Test with the existing --nested-test mode first, and confirm the Start menu, taskbar, Files, wallpaper, security and logout.

4. As the account which will use Melania, explicitly register its private checkout:

    python3 scripts/manage-melania-labwc-login.py --register-user

   This validates the build and writes a private XDG user registration file; it is NOT a system installation.

5. Only after testing, explicitly add a new login-manager choice as administrator:

    sudo python3 scripts/manage-melania-labwc-login.py --install-system

   Exactly two new system paths are used: /usr/local/libexec/melania-labwc-launch and /usr/share/wayland-sessions/org.melaniadesktop.Labwc.desktop. Other installed desktop entries are untouched.

6. Log out and select Melania Desktop (Labwc) in your login screen. Keep Cinnamon/XFCE as a selectable fallback and use a spare account for the first independent login test.

## Rollback

As each user:

    python3 scripts/manage-melania-labwc-login.py --unregister-user

To remove the optional login choice for all users:

    sudo python3 scripts/manage-melania-labwc-login.py --uninstall-system

The --status mode inspects registration without modification. Install and uninstall are idempotent and reject symlinks and foreign files. Uninstall validates both destinations before touching either.

## Security and limitations

- Only the root-owned generic launcher is visible system-wide. The registration file under ~/.config/melania/labwc-login-root has mode 0600 inside a mode-0700 Melania config directory; the user checkout is not referenced by the global desktop entry.
- The root-owned wrapper runs as the login account and invokes the previously tested Melania session entry with --run, with no eval or shell-command construction.
- Register-user is not allowed with sudo. All system changes require an explicit separate administrator action.
- Running this process never changes the default login choice, existing desktop config, native Rust compositor readiness or host services.
- The CI tests use a disposable fake filesystem root and user config and exercise real generic Bash launching, collisions, symlinks and idempotent removal. Nothing is installed in the CI runner's /usr.
- Real physical hardware login, multi-monitor operation, suspend/resume, extensive X11 application support and long-duration stability are still separate acceptance gates. This is a controlled opt-in, NOT certification of full XFCE parity.
