#!/usr/bin/env bash
# Safe, opt-in compositor development sandbox. Never replaces the login session.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
UPSTREAM="$ROOT/.build/smithay-v0.7.0"
if ! command -v git >/dev/null || ! command -v cargo >/dev/null; then
  echo 'Servono git, Rust e Cargo.' >&2; exit 1
fi
if [ -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
  echo 'Serve una sessione grafica host X11/Wayland per il test annidato.' >&2; exit 1
fi
if [ ! -d "$UPSTREAM/.git" ]; then
  mkdir -p "$(dirname "$UPSTREAM")"
  echo 'Scaricamento Smithay v0.7.0; serve accesso Internet.'
  git clone --depth 1 --branch v0.7.0 https://github.com/Smithay/smithay.git "$UPSTREAM"
fi
actual="$(git -C "$UPSTREAM" describe --tags --exact-match 2>/dev/null || true)"
if [ "$actual" != 'v0.7.0' ]; then
  echo "Versione Smithay inattesa: $actual" >&2; exit 1
fi
printf 'Compilazione Melania Preview GTK 4...\n'
cargo build --manifest-path "$ROOT/Cargo.toml" -p melania-preview
printf 'Avvio del compositore demo upstream Smithay/smallvil in finestra annidata...\n'
printf 'NOTA: questo NON è ancora il compositore proprietario di Melania.\n'
printf 'Esci chiudendo la finestra di smallvil o con Ctrl+C nel terminale.\n'
cd "$UPSTREAM"
# The -c command starts an actual Wayland client under the nested compositor.
cargo run -p smallvil -- -c "$ROOT/target/debug/melania-preview"
