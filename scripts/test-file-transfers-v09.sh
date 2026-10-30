#!/usr/bin/env bash
# Integration test for melania-filectl (uses ONLY a newly created temp directory).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
command -v cargo >/dev/null || { echo 'Manca cargo: installa Rust prima.' >&2; exit 127; }
tmp="$(mktemp -d)"
trap 'rm -rf -- "$tmp"' EXIT
mkdir -p "$tmp/origin" "$tmp/dest"
printf 'melania v0.9\n' > "$tmp/origin/doc.txt"
# The default must not modify anything.
cargo run -q -p melania-filectl -- copy "$tmp/dest" "$tmp/origin/doc.txt"
test ! -e "$tmp/dest/doc.txt"
cargo run -q -p melania-filectl -- --apply copy "$tmp/dest" "$tmp/origin/doc.txt"
test -f "$tmp/origin/doc.txt" && cmp "$tmp/origin/doc.txt" "$tmp/dest/doc.txt"
# Existing target must not be overwritten.
if cargo run -q -p melania-filectl -- --apply copy "$tmp/dest" "$tmp/origin/doc.txt"; then
  echo 'ERRORE: copia con conflitto accettata' >&2; exit 1
fi
mkdir -p "$tmp/dest2"
cargo run -q -p melania-filectl -- --apply move "$tmp/dest2" "$tmp/origin/doc.txt"
test ! -e "$tmp/origin/doc.txt" && test -f "$tmp/dest2/doc.txt"
# Moving onto an existing target must fail without deleting the original.
printf 'originale\n' > "$tmp/origin/doc.txt"
if cargo run -q -p melania-filectl -- --apply move "$tmp/dest2" "$tmp/origin/doc.txt"; then
  echo 'ERRORE: move con conflitto accettato' >&2; exit 1
fi
grep -qx 'originale' "$tmp/origin/doc.txt"
grep -qx 'melania v0.9' "$tmp/dest2/doc.txt"
echo 'PASS: anteprima, copia, copia senza sovrascrittura, move atomico, conflitti'
