# Melania v0.13 — Compiler verification gate

This is a **diagnostic release**, not a compiled desktop environment.

## Why this release matters
The previous smoke test accepted a live Wayland socket and an IPC response even if no application window appeared. It now **requires a real Melania Files GTK4 toplevel** in the compositor's window list. This prevents a false positive.

## On a Debian/Ubuntu/Mint development machine

1. Install a recent Rust toolchain (prefer rustup), and native dependencies as described in README.
2. From the repository root, run `./scripts/diagnose-on-linux.sh`.
3. Examine `reports/compile-output.log` and `reports/*-build.log`.
4. If compilation passes, run `xvfb-run -a ./scripts/smoke-nested-wayland.sh` (requires Xvfb and xauth).
5. Report any failing logs before attempting an ordinary login session.

The smoke test intentionally cannot be passed when `melania-files` is missing, or when the compositor does not register its real window.

## Status in the current container
- Cargo / rustc unavailable.
- GTK4, Wayland and EGL development packages unavailable.
- External package endpoints not resolvable.
- Therefore no Rust compilation or native Wayland runtime claim is made.
- Python offline audit, release invariants and filesystem prototype unit tests run locally.

## Drag and drop
The prototype's local filesystem transfer remains COPY-only; no source deletion on drag. Full Wayland data-device transfer including GTK/Qt compatibility is unverified.
