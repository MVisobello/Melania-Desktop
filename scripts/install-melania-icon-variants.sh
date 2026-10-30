#!/usr/bin/env bash
# Install 3 Melania icon themes in the current user's freedesktop icon folder.
# Intentionally does not alter host GTK/GNOME preferences.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE="${1:-$ROOT/dist/melania-icon-themes}"
BASE="${XDG_DATA_HOME:-$HOME/.local/share}/icons"
THEMES=(Melania-Light Melania-Dark Melania-Light-on-Dark-Panel)
case "${1:-}" in
  --help|-h)
    echo "Uso: $0 [cartella-contenente-i-tre-temi]"
    echo "Non modifica il tema corrente del desktop host."
    exit 0 ;;
esac
for name in "${THEMES[@]}"; do
    [[ -f "$SOURCE/$name/index.theme" ]] || {
      echo "Tema mancante: $SOURCE/$name/index.theme" >&2; exit 1;
    }
done
mkdir -p "$BASE"
stage="$(mktemp -d "$BASE/.melania-icons-XXXXXX")"
trap 'rm -rf "$stage"' EXIT
# Copy all themes together so their relative symlinks stay valid.
for name in "${THEMES[@]}"; do
    cp -a "$SOURCE/$name" "$stage/$name"
done
for name in "${THEMES[@]}"; do
    if [[ -e "$BASE/$name/index.theme" ]] &&
       ! grep -qx "Name=${name//-/ }" "$BASE/$name/index.theme"; then
        echo "Rifiuto di sovrascrivere un tema non Melania: $BASE/$name" >&2
        exit 2
    fi
done
# Verify inter-theme symbolic links remain resolved *after* installation.
for name in "${THEMES[@]}"; do
    [[ -f "$stage/$name/index.theme" ]] || exit 3
done
for name in "${THEMES[@]}"; do
    rm -rf -- "$BASE/$name"
    mv -- "$stage/$name" "$BASE/$name"
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -f "$BASE/$name" >/dev/null 2>&1 || true
    fi
done
echo "Installati 3 temi Melania in $BASE"
echo "Seleziona Light, Dark o Light on Dark Panel nelle Impostazioni di Melania."
echo "Preferenze del desktop Linux ospitante lasciate invariate."
