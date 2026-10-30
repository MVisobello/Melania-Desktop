# Local results — 29 October 2026

- `python3 scripts/audit-build.py --json reports/offline-audit.json`: **PASS**, 13/13 manifests and declared local dependencies validated.
- `bash -n` on new shell scripts: **PASS**.
- `python3 -m unittest discover -s preview-lab -p test_preview.py -v`: **PASS**, 5 tests for the legacy X11/Tkinter preview file operations.
- `./scripts/test-preview-lab.sh reports/ux-x11-proof.png`: **PASS**, Xvfb 1280×800 real mouse drag & drop, file copy and source preservation; quick-setting demo responded. Not the Rust compositor.
- `./scripts/verify-rust-linux.sh`: **BLOCKED (exit code 3)** because Cargo is not installed.
- Native development libs for GTK4/Layer Shell: **UNAVAILABLE** in this runtime.
- `apt-get update` and Rust distribution endpoints: **BLOCKED** by DNS resolution.
- Rust type checking, Rust tests, Rust GTK execution and nested Wayland compositor: **NOT EXECUTED**.

Don't confuse these labels; first Wayland acceptance requires a green actual compiler + Wayland smoke run.
