#!/usr/bin/env bash
# Optional per-user preview launcher. Never installs into wayland-sessions.
# Invocation: bash scripts/install-melania-preview-launcher.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$HOME/.local/bin"
DATA="$HOME/.local/share"
if [[ -v XDG_DATA_HOME && -n "$XDG_DATA_HOME" ]]; then DATA="$XDG_DATA_HOME"; fi
APPS="$DATA/applications"
mkdir -p "$BIN" "$APPS"
TARGET="$BIN/melania-session"
DESKTOP="$APPS/org.melaniadesktop.Nested.desktop"
if [[ -e "$TARGET" && ! -f "$TARGET" ]]; then
    echo "BLOCKED: $TARGET is not a regular file" >&2; exit 4
fi
# Never silently replace a different launcher or symlink.
if [[ -e "$TARGET" ]] && ! grep -Fq "$ROOT/scripts/melania-session" "$TARGET"; then
    echo "BLOCKED: an unrelated executable already exists at $TARGET" >&2; exit 4
fi
TMP="$(mktemp "$BIN/.melania-session-XXXXXX")"
trap 'rm -f -- "$TMP"' EXIT
printf '#!/usr/bin/env bash\nexec /usr/bin/env bash %q "$@"\n' "$ROOT/scripts/melania-session" > "$TMP"
chmod 700 "$TMP"
mv -f -- "$TMP" "$TARGET"
trap - EXIT
# Escape only for Desktop Entry Exec quoting. This is NOT shell evaluation.
ESCAPED="$(printf '%s' "$TARGET" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' -e 's/%/%%/g')"
if [[ -e "$DESKTOP" ]] && ! grep -q '^X-Melania-Nested-Preview=true$' "$DESKTOP"; then
    echo "BLOCKED: existing unrelated desktop entry at $DESKTOP" >&2; exit 4
fi
DESKTOP_TMP="$(mktemp "$APPS/.melania-preview-XXXXXX")"
trap 'rm -f -- "$DESKTOP_TMP"' EXIT
cat > "$DESKTOP_TMP" <<EOF
[Desktop Entry]
Type=Application
Version=1.0
Name=Melania Desktop (anteprima annidata)
Comment=Prova isolata dentro la sessione Linux esistente, non è una sessione di login
Exec="$ESCAPED" --nested
Icon=melania
Terminal=false
Categories=System;Utility;
X-Melania-Nested-Preview=true
EOF
chmod 600 "$DESKTOP_TMP"
mv -f -- "$DESKTOP_TMP" "$DESKTOP"
trap - EXIT
echo "Installed local nested-preview launcher: $DESKTOP"
echo 'Native login session NOT installed (DRM/KMS backend still pending).'
