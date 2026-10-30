#!/usr/bin/env bash
# Execute a window command from another terminal in the same login session.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
[[ -d "${XDG_RUNTIME_DIR:-/nonexistent}" ]] || { echo 'XDG_RUNTIME_DIR mancante' >&2; exit 2; }
shopt -s nullglob
sockets=("$XDG_RUNTIME_DIR"/melania-*-control.sock)
if (( ${#sockets[@]} != 1 )); then
  echo "Trovate ${#sockets[@]} sessioni Melania (attesa esattamente una)." >&2
  exit 2
fi
export MELANIA_CONTROL_SOCKET="${sockets[0]}"
cd "$ROOT"
exec cargo run --quiet -p melania-control --bin melania-winctl -- "$@"
