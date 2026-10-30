#!/usr/bin/env bash
# Melania Aero icon theme installer. Default: install to user's XDG data dir.
# Does not modify global icon/theme settings unless --activate is explicitly passed.
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
SOURCE="$PROJECT_DIR/branding/Melania"
BASE="${XDG_DATA_HOME:-${HOME:?}/.local/share}/icons"
TARGET="$BASE/Melania"
ACTIVATE=0
MODE=install
for arg in "$@"; do
    case "$arg" in
        --activate) ACTIVATE=1 ;;
        --check) MODE=check ;;
        --help|-h)
            cat <<'EOF'
Uso: ./scripts/install-melania-icons.sh [--activate|--check]
  (nessun parametro): installa solo il tema icone nella cartella dell'utente.
  --activate: dopo l'installazione imposta il tema nel desktop attuale tramite GSettings.
  --check: controlla che il tema sia presente (non modifica niente).
Nota: --activate modifica le preferenze del desktop che stai utilizzando.
EOF
            exit 0 ;;
        *) echo "Parametro sconosciuto: $arg" >&2; exit 2 ;;
    esac
done
if [[ "$MODE" == check ]]; then
    if [[ -f "$TARGET/index.theme" ]] && grep -q '^Name=Melania Aero$' "$TARGET/index.theme"; then
        printf 'Tema Melania Aero installato: %s\n' "$TARGET"
    else
        echo 'Tema non ancora installato' >&2; exit 1
    fi
    exit 0
fi
[[ -f "$SOURCE/index.theme" ]] || { echo 'Tema sorgente mancante: eseguire tools/build_icon_theme.py' >&2; exit 1; }
mkdir -p -- "$BASE"
# Refuse to overwrite an unrelated theme with the same directory name.
if [[ -e "$TARGET/index.theme" ]] && ! grep -q '^Name=Melania Aero$' "$TARGET/index.theme"; then
    echo 'Esiste già un tema diverso chiamato Melania. Nessun file modificato.' >&2; exit 1
fi
# Stage and swap; failures during copy leave the previous install intact.
STAGE="$(mktemp -d "$BASE/.melania-stage.XXXXXX")"
trap 'rm -rf -- "$STAGE"' EXIT
cp -R -- "$SOURCE/." "$STAGE/"
if [[ -d "$TARGET" ]]; then
    # Update only the known Melania directory, never another theme.
    rm -rf -- "$TARGET"
fi
mv -- "$STAGE" "$TARGET"
trap - EXIT
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f "$TARGET" >/dev/null 2>&1 || true
fi
printf 'Tema installato in %s\n' "$TARGET"
if (( ACTIVATE )); then
    if command -v gsettings >/dev/null 2>&1 && gsettings writable org.gnome.desktop.interface icon-theme | grep -qx true; then
        gsettings set org.gnome.desktop.interface icon-theme Melania
        echo 'Tema Melania attivato nelle preferenze GTK correnti.'
    else
        echo 'Tema installato. Non è possibile attivarlo automaticamente su questo desktop.' >&2
    fi
else
    echo 'Preferenze del desktop lasciate invariate. Per attivare: aggiungere --activate.'
fi
