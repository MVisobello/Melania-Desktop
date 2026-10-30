#!/usr/bin/env bash
# One-command user diagnostics. Never installs packages or changes active session.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p reports
OUT="reports/linux-diagnostics.txt"
{
  printf 'Melania validation %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf 'Kernel: '; uname -srmo
  printf 'Cargo: '; (command -v cargo && cargo --version) || printf 'MISSING\n'
  printf 'Rustc: '; (command -v rustc && rustc --version) || printf 'MISSING\n'
  printf 'GTK4: '; pkg-config --modversion gtk4 2>/dev/null || printf 'MISSING\n'
  printf 'GTK layer shell: '; pkg-config --modversion gtk4-layer-shell 2>/dev/null || printf 'MISSING\n'
  printf 'Wayland client library: '; pkg-config --modversion wayland-client 2>/dev/null || printf 'MISSING\n'
  printf 'Xvfb: '; command -v Xvfb || printf 'MISSING\n'
} > "$OUT"
python3 scripts/audit-build.py --json reports/offline-audit.json >>"$OUT" 2>&1
AUDIT=$?
python3 -m unittest discover -s tests -p 'test_release_invariants.py' -v >reports/static-tests.log 2>&1
STATIC=$?
if ! command -v cargo >/dev/null 2>&1; then
  printf 'BLOCKED: Cargo not installed; real Rust compilation NOT RUN.\n' >> "$OUT"
  cp "$OUT" reports/build-status.txt
  echo 'Compilation blocked; see reports/linux-diagnostics.txt. Install a recent Rust toolchain and GTK4 SDK.'
  exit 3
fi
bash scripts/verify-rust-linux.sh > reports/compile-output.log 2>&1
BUILD=$?
printf 'audit=%s static=%s cargo_compile=%s\n' "$AUDIT" "$STATIC" "$BUILD" >> "$OUT"
if (( AUDIT || STATIC || BUILD )); then
  echo 'Some validation gates failed; review reports/*.log.'
  exit 1
fi
echo 'All available compilation checks passed; use a graphical Linux session to run nested compositor smoke test.'
