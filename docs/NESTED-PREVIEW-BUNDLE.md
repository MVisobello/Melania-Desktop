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

## Linux Mint 22 / Ubuntu 24.04: missing GTK4 layer-shell runtime

If \`run.sh --check\` reports \`libgtk4-layer-shell.so.0 => not found\`,
the compositor can start while its desktop and panel cannot. Unlike the
similarly named GTK3 library \`libgtk-layer-shell0\`, Melania requires the
**GTK4** library.

First check whether your distribution offers the runtime:

\`\`\`sh
apt-cache policy libgtk4-layer-shell0
sudo apt install libgtk4-layer-shell0  # only when APT shows an available candidate
\`\`\`

Ubuntu 24.04 and Linux Mint 22 often lack that package in standard
repositories. Avoid mixing system repositories from newer Ubuntu releases.
Instead, build the upstream GTK4 layer-shell v1.0.4 locally:

\`\`\`sh
sudo apt install git meson ninja-build build-essential pkg-config \
    libgtk-4-dev libwayland-dev wayland-protocols
cd "$HOME"
git clone --depth 1 --branch v1.0.4 \
    https://github.com/wmww/gtk4-layer-shell.git
cd gtk4-layer-shell
meson setup build --prefix="$HOME/.local/melania-deps" --libdir=lib \
    -Dexamples=false -Ddocs=false -Dtests=false \
    -Dintrospection=false -Dvapi=false
meson compile -C build
meson install -C build
\`\`\`

Return to the unpacked Melania preview directory. For both \`--check\`
and normal launch, make the local runtime visible only to that process:

\`\`\`sh
LD_LIBRARY_PATH="$HOME/.local/melania-deps/lib\${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" ./run.sh --check
LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe \
  LD_LIBRARY_PATH="$HOME/.local/melania-deps/lib\${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" ./run.sh
\`\`\`

No \`sudo\`, \`ldconfig\` or system-wide \`LD_LIBRARY_PATH\` change is
needed for this local build. The VirtualBox \`libEGL\`/DRI3/ZINK messages
are separate GPU acceleration diagnostics; a software rendering override
may help, at the cost of reduced graphics performance.

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
