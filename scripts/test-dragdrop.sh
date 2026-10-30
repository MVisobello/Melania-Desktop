#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
command -v cargo >/dev/null || { echo 'Cargo non installato' >&2; exit 1; }
command -v pkg-config >/dev/null && pkg-config --exists gtk4 || { echo 'GTK4 dev non disponibile' >&2; exit 1; }
cargo test -p melania-transfer -p melania-dnd
cargo build -p melania-files
LAB="$(mktemp -d "${TMPDIR:-/tmp}/melania-dnd.XXXXXXXX")"
trap 'rm -rf -- "$LAB"' EXIT
mkdir -p "$LAB/Cartella destinazione" "$LAB/Documenti di esempio"
printf 'Melania Desktop: prova drag & drop\n' > "$LAB/Documento prova.txt"
printf 'Contenuto dentro una directory\n' > "$LAB/Documenti di esempio/nota.txt"
printf '\nLAB TEMPORANEO: %s\n' "$LAB"
printf '%s\n' \
 '1. Trascina «Documento prova.txt» sulla cartella «Cartella destinazione».' \
 '2. Lascialo senza modificatori per COPIA; prova Maiusc per SPOSTA.' \
 '3. Il sistema non sovrascrive file già esistenti.' \
 '4. Trascina anche «Documenti di esempio» nella cartella di destinazione.' \
 '5. Chiudi la finestra Melania Files per rimuovere il laboratorio temporaneo.'
"$ROOT/target/debug/melania-files" "$LAB"
printf '\nContenuto della destinazione dopo il test:\n'
find "$LAB/Cartella destinazione" -maxdepth 2 -print || true
