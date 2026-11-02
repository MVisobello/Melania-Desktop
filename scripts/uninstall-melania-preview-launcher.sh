#!/usr/bin/env bash
# Remove only the launcher previously installed by this exact checkout.
# This never touches login-manager sessions, other .desktop entries or binaries.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$HOME/.local/bin"
DATA="$HOME/.local/share"
if [[ -v XDG_DATA_HOME && -n "$XDG_DATA_HOME" ]]; then DATA="$XDG_DATA_HOME"; fi
TARGET="$BIN/melania-session"
DESKTOP="$DATA/applications/org.melaniadesktop.Nested.desktop"

# Validate *both* paths before deleting either one. Do not follow symlinks,
# modify files left by other applications or remove parent directories.
for path in "$TARGET" "$DESKTOP"; do
  if [[ -L "$path" || ( -e "$path" && ! -f "$path" ) ]]; then
    echo "BLOCKED: launcher path is a symlink or not a regular file: $path" >&2
    exit 4
  fi
done

if [[ -f "$TARGET" ]]; then
  EXPECTED="$(printf '#!/usr/bin/env bash\nexec /usr/bin/env bash %q "$@"\n' "$ROOT/scripts/melania-session")"
  if [[ "$(cat -- "$TARGET")" != "$EXPECTED" ]]; then
    echo "BLOCKED: not a Melania launcher from this checkout: $TARGET" >&2
    exit 4
  fi
fi
if [[ -f "$DESKTOP" ]]; then
  ESCAPED="$(printf '%s' "$TARGET" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' -e 's/%/%%/g')"
  if ! grep -Fxq 'X-Melania-Nested-Preview=true' "$DESKTOP" || \
     ! grep -Fxq "Exec=\"$ESCAPED\" --nested" "$DESKTOP"; then
    echo "BLOCKED: desktop entry is not the expected nested preview: $DESKTOP" >&2
    exit 4
  fi
fi

rm -f -- "$DESKTOP" "$TARGET"
echo 'Melania nested-preview launcher removed (or was already absent).'
echo 'The host Linux session and all system login-manager entries were left untouched.'
