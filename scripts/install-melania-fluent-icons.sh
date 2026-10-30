#!/usr/bin/env bash
# Install the three original Melania Fluent icon themes for this user only.
# Does not change the host's GTK/GNOME settings.
set -euo pipefail
SOURCE="${1:-$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/../dist/melania-fluent-icons}"
BASE="${XDG_DATA_HOME:-$HOME/.local/share}/icons"
THEMES=(Melania-Fluent-Light Melania-Fluent-Dark Melania-Fluent-Light-on-Dark-Panel)
for theme in "${THEMES[@]}"; do
  [[ -f "$SOURCE/$theme/index.theme" ]] || { echo "Tema mancante: $theme" >&2; exit 1; }
done
mkdir -p "$BASE"
for theme in "${THEMES[@]}"; do
  if [[ -e "$BASE/$theme/index.theme" ]] &&
     ! grep -q '^Comment=Original Melania Fluent icons' "$BASE/$theme/index.theme"; then
    echo "Rifiuto di sostituire il tema estraneo $BASE/$theme" >&2; exit 2
  fi
done
stage=$(mktemp -d "$BASE/.melania-fluent-XXXXXX")
trap 'rm -rf -- "$stage"' EXIT
for theme in "${THEMES[@]}"; do cp -a "$SOURCE/$theme" "$stage/$theme"; done
for theme in "${THEMES[@]}"; do
  rm -rf -- "$BASE/$theme"
  mv -- "$stage/$theme" "$BASE/$theme"
done
for theme in "${THEMES[@]}"; do
  test -f "$BASE/$theme/places/scalable/folder.svg" || {
    echo "Collegamenti non risolti in $theme" >&2; exit 3;
  }
done
echo "Temi Melania Fluent installati in $BASE"
echo "Seleziona il tema dalle Impostazioni Aspetto di Melania."
