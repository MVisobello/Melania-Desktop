# Melania — build validation report and release gate

## Scope
This is a build verification package based on **v0.12**, not a new verified DE release.
It separates four types of evidence:

1. Offline manifest/asset audit (`scripts/audit-build.py`).
2. GTK-independent legacy UX preview tests (`python3 -m unittest …`).
3. Actual Rust compilation/tests (`scripts/verify-rust-linux.sh`) — **not run in ChatGPT environment** due to unavailable toolchain and DNS.
4. Actual nested Wayland compositor socket/IPC startup (`scripts/smoke-nested-wayland.sh`) — **not run here**; requires built binaries plus X11/Wayland host.

A GREEN offline audit DOES NOT imply any Rust code compiles.

## Linux Mint / Ubuntu commands

Do not run with `sudo` except for dependency installation. Never run the experimental compositor as your primary session.

```bash
sudo apt update
sudo apt install -y git curl build-essential pkg-config libgtk-4-dev \
    libgtk4-layer-shell-dev libwayland-dev libxkbcommon-dev \
    libegl1-mesa-dev libgles2-mesa-dev libx11-dev \
    libudev-dev libseat-dev libdrm-dev libgbm-dev libdbus-1-dev \
    xvfb xauth
```

Install **current stable Rust** from https://rustup.rs if your distro's rustc is older than 1.85 (gtk4-layer-shell 0.8.1 requires Rust 1.85 or newer). Follow its official installer instructions.

```bash
python3 scripts/audit-build.py --json reports/offline-audit.json
./scripts/verify-rust-linux.sh
cargo build -p melania-compositor -p melania-control -p melania-files
xvfb-run -a ./scripts/smoke-nested-wayland.sh
```

The last command deliberately tests **only compositor startup, socket creation and IPC responsiveness**. It does not test rendering correctness, actual file drops between GTK and Qt, Wayland popups or XWayland support.

## GitHub Actions

When this repository is uploaded to GitHub, `.github/workflows/linux-verify.yml` runs separate steps for Rust-only modules, GTK applications and Smithay. Compiler errors are preserved as build artifacts, so they can be corrected systematically. A separate job retains the X11 Tkinter-based demo tests to avoid confusing it with Wayland.

## Known blockers in ChatGPT runtime (2026-10-29)

- `cargo`: unavailable
- `rustc`: unavailable
- `pkg-config gtk4`: unavailable
- `pkg-config gtk4-layer-shell`: unavailable
- `apt-get update`: failed DNS resolving deb.debian.org
- `curl https://static.rust-lang.org`: failed DNS resolution
- Rust compilation: **NOT EXECUTED**
- GTK 4 app runtime and Wayland compositor: **NOT EXECUTED**

## Minimum acceptance criteria for first verified Melania Wayland session

- All Rust build jobs GREEN.
- Nested compositor remains alive and creates a private socket.
- `melania-winctl list` succeeds against compositor.
- Real GTK Files window appears and responds to focus/move/resize.
- Real two-process GTK drag and drop tested with temporary files.
- Source file remains untouched on COPY and target contents match.
- Snapshot/screenshot captured from a Linux test environment.
- No automated changes to login manager or user's desktop session.
