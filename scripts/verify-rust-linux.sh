#!/usr/bin/env bash
# Three distinct compilation gates; a failure remains a failure.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p reports
python3 scripts/audit-build.py --json reports/offline-audit.json
if ! command -v cargo >/dev/null 2>&1; then
  echo 'BLOCKED: cargo not installed; compilation NOT EXECUTED.' >&2
  exit 3
fi
set +e
cargo check -p melania-core -p melania-shell-model -p melania-dnd -p melania-transfer -p melania-control -p melania-config -p melania-services -p melania-filectl --all-targets 2>&1 | tee reports/core-build.log
core_status=${PIPESTATUS[0]}
cargo test -p melania-core -p melania-shell-model -p melania-dnd -p melania-transfer -p melania-control -p melania-config -p melania-services -p melania-filectl 2>&1 | tee reports/core-tests.log
test_status=${PIPESTATUS[0]}
cargo check -p melania-compositor --all-targets 2>&1 | tee reports/compositor-build.log
compositor_status=${PIPESTATUS[0]}
if command -v pkg-config >/dev/null 2>&1 && pkg-config --exists gtk4 gtk4-layer-shell; then
  cargo check -p melania-files -p melania-settings -p melania-preview -p melania-panel --all-targets 2>&1 | tee reports/gtk-build.log
  gtk_status=${PIPESTATUS[0]}
else
  echo "BLOCKED: missing GTK4 or GTK4 Layer Shell development libraries" | tee reports/gtk-build.log
  gtk_status=3
fi
set -e
printf '\nRust build outcomes: core=%d tests=%d gtk=%d compositor=%d\n' "$core_status" "$test_status" "$gtk_status" "$compositor_status"
if (( core_status || test_status || gtk_status || compositor_status )); then
  echo 'FAIL: see reports/*.log for exact compiler errors.' >&2
  exit 1
fi
echo 'PASS: all Rust build gates passed.'
