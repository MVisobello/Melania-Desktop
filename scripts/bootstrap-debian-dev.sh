#!/usr/bin/env bash
# Explicit install on Debian-based development machines only. Does NOT change login session.
set -euo pipefail
if [[ "${1:-}" != '--install' ]]; then
  echo 'Uso: ./scripts/bootstrap-debian-dev.sh --install'
  echo 'Installa Rust/Cargo (dal repository della distribuzione), GTK4, Smithay prerequisites, and Xvfb.'
  echo 'Non modifica la sessione di login né i file personali.'
  exit 2
fi
sudo apt-get update
sudo apt-get install -y cargo rustc build-essential pkg-config libgtk-4-dev \
  libgtk4-layer-shell-dev libwayland-dev libxkbcommon-dev libegl1-mesa-dev \
  libgles2-mesa-dev libx11-dev libudev-dev libseat-dev libdrm-dev \
  libgbm-dev libdbus-1-dev xvfb python3-tk python3-pil python3-pip
printf '%s\n' 'Dipendenze installate. Avvia cargo check --workspace e cargo test --workspace.'
