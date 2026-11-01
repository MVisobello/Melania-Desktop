#!/usr/bin/env bash
# Real Rust transfers on synthetic temporary data only; never touch user files.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FILECTL="$ROOT/target/debug/melania-filectl"
if [[ -v MELANIA_FILECTL && -n "$MELANIA_FILECTL" ]]; then FILECTL="$MELANIA_FILECTL"; fi
[[ -x "$FILECTL" ]] || { echo 'BLOCKED: build melania-filectl first' >&2; exit 3; }
TEMP="$(mktemp -d -t melania-safe-files-XXXXXXXX)"
trap 'rm -rf -- "$TEMP"' EXIT
mkdir -p "$TEMP/source" "$TEMP/dest" "$TEMP/another"
printf 'Melania copy regression\n' > "$TEMP/source/Documento prova.txt"
printf 'Do not overwrite\n' > "$TEMP/dest/keep.txt"
ln -s /etc "$TEMP/source/link-to-etc"
BEFORE="$(sha256sum "$TEMP/source/Documento prova.txt" | cut -d' ' -f1)"

# Without --apply the CLI must not change the destination.
"$FILECTL" copy "$TEMP/dest" "$TEMP/source/Documento prova.txt" > "$TEMP/dry-run.log"
grep -q 'ANTEPRIMA:' "$TEMP/dry-run.log"
[[ ! -e "$TEMP/dest/Documento prova.txt" ]] || { echo 'FAIL: dry run wrote a file' >&2; exit 1; }

"$FILECTL" --apply copy "$TEMP/dest" "$TEMP/source/Documento prova.txt"
cmp "$TEMP/source/Documento prova.txt" "$TEMP/dest/Documento prova.txt"
[[ "$(sha256sum "$TEMP/dest/Documento prova.txt" | cut -d' ' -f1)" == "$BEFORE" ]]
if "$FILECTL" --apply copy "$TEMP/dest" "$TEMP/source/Documento prova.txt"; then
    echo 'FAIL: overwrite was allowed' >&2; exit 1
fi
[[ "$(sha256sum "$TEMP/dest/Documento prova.txt" | cut -d' ' -f1)" == "$BEFORE" ]]
[[ "$(cat "$TEMP/dest/keep.txt")" == "Do not overwrite" ]]

# Symlink stays a link; no recursive traversal of the external target.
"$FILECTL" --apply copy "$TEMP/dest" "$TEMP/source/link-to-etc"
[[ -L "$TEMP/dest/link-to-etc" ]]
[[ "$(readlink "$TEMP/dest/link-to-etc")" == "/etc" ]]

"$FILECTL" --apply move "$TEMP/another" "$TEMP/source/Documento prova.txt"
[[ ! -e "$TEMP/source/Documento prova.txt" ]]
[[ "$(sha256sum "$TEMP/another/Documento prova.txt" | cut -d' ' -f1)" == "$BEFORE" ]]
printf 'another version\n' > "$TEMP/source/Documento prova.txt"
if "$FILECTL" --apply move "$TEMP/another" "$TEMP/source/Documento prova.txt"; then
    echo 'FAIL: MOVE replaced target' >&2; exit 1
fi
[[ "$(cat "$TEMP/source/Documento prova.txt")" == "another version" ]]
[[ "$(sha256sum "$TEMP/another/Documento prova.txt" | cut -d' ' -f1)" == "$BEFORE" ]]
echo 'PASS: actual Rust file transfers, collision protection and atomic MOVE'
