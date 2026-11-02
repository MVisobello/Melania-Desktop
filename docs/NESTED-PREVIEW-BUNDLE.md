# Melania Desktop — portable nested development preview (Linux)

This archive contains **compiled Linux binaries**, not a standalone desktop
environment or native DRM/KMS login session. It runs inside an already-open
X11/Wayland desktop, in a separate window. It never installs a
`wayland-sessions`/`xsessions` entry or replaces your system's login manager.

## Intended platform

The CI artifact is built on **Ubuntu 26.04 x86_64** and depends on compatible
Linux GTK4, gtk4-layer-shell, Wayland, Mesa, X11, libinput and D-Bus runtime
libraries. It is not an AppImage or fully statically linked executable.
Prefer a disposable Ubuntu 26.04 VM with a working graphical desktop.
Other Linux distributions may require different libraries and are not certified.

## Download, verify and run (inside a Linux graphical session)

Download the `melania-nested-preview` artifact from the GitHub Actions
"Melania nested preview bundle" workflow. GitHub artifacts are ZIP wrappers
containing the files listed here.

```sh
sha256sum -c SHA256SUMS
tar -xzf melania-nested-preview.tar.gz
cd melania-nested-preview
./run.sh --check
./run.sh
```

Extract with a normal non-root user into a private working directory.
The bundled `manifest.json` records the exact source commit and SHA-256
hashes of each shipped binary/resource. The archive's `SHA256SUMS` file
verifies the compressed archive. Verify you trust the release's commit and
the GitHub Actions source before executing downloaded programs.

## What works and what is still experimental

The nested compositor launches the desktop surface, taskbar, Melania Files,
Melania Settings and notification daemon from `bin/`. The bundle includes
its default wallpaper and runs against GTK/Wayland libraries on the host.
The bundled `run.sh` does **not** call Cargo or require a source checkout.

The full nested desktop, input and real-pixel integration workflow checks
are independent evidence; success in CI does not prove every real GPU,
multi-monitor, HiDPI, GVfs backend or theme variation. Expect experimental
behaviour and keep the host desktop available.

**Do not use this build as your only desktop.** Native login, libseat,
hardware DRM/KMS, VT switching and recovery are not certified. The QEMU KMS
`TEST_ONLY` tests do not change that.

## Removal / rollback

Quit the nested desktop (or close its host window) and delete the extracted
`melania-nested-preview` directory. No system session is installed.
You do not need `sudo` to extract, run or remove the preview.

The source checkout also provides optional per-user application menu launcher
install/uninstall scripts; they are **not part of this portable archive**.

## Problems and evidence

Run `./run.sh --check` for missing-host/bundle prerequisites. Report the
`manifest.json` commit, distribution version, GPU, terminal output and the
relevant GitHub Actions run URLs. Never treat "runner started" as equivalent
to "end-to-end tests passed".
