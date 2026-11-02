#!/usr/bin/env bash
# Build the GTK4 layer-shell C dependency in a private prefix for Mint 22.x.
# No sudo, no /usr/local writes. Pin the upstream source revision for review.
set -euo pipefail

UPSTREAM="https://github.com/wmww/gtk4-layer-shell.git"
UPSTREAM_TAG="v1.0.4"
UPSTREAM_COMMIT="536ff516ed68b9bb34afc4c07f942a54b2b4b03f"

usage() {
  echo 'Usage: bash scripts/build-mint-layer-shell.sh /absolute/private/prefix' >&2
  exit 2
}
[[ $# -eq 1 ]] || usage
PREFIX="$1"
[[ "$PREFIX" = /* ]] || { echo 'BLOCKED: prefix must be absolute' >&2; exit 3; }
[[ "$PREFIX" != /usr* && "$PREFIX" != /lib* && "$PREFIX" != /etc* ]] || {
  echo 'BLOCKED: never install a vendored library into system directories' >&2
  exit 3
}
for tool in git meson ninja pkg-config; do
  command -v "$tool" >/dev/null || {
    echo "BLOCKED: missing build tool $tool" >&2; exit 3;
  }
done
pkg-config --atleast-version=4.8 gtk4 || {
  echo 'BLOCKED: GTK4 >= 4.8 development library required' >&2; exit 3;
}
temp="$(mktemp -d -t melania-mint-layer-shell-XXXXXX)"
trap 'rm -rf -- "$temp"' EXIT
git clone --depth 1 --branch "$UPSTREAM_TAG" "$UPSTREAM" "$temp/source"
actual="$(git -C "$temp/source" rev-parse HEAD)"
[[ "$actual" == "$UPSTREAM_COMMIT" ]] || {
  echo "BLOCKED: upstream source checksum changed: $actual" >&2; exit 3;
}
meson setup "$temp/build" "$temp/source" \
  --prefix "$PREFIX" --libdir lib --buildtype release \
  -Dexamples=false -Ddocs=false -Dtests=false \
  -Dsmoke-tests=false -Dintrospection=false -Dvapi=false
ninja -C "$temp/build"
meson install -C "$temp/build" --no-rebuild
test -s "$PREFIX/lib/libgtk4-layer-shell.so.0.0.0"
test -s "$PREFIX/lib/pkgconfig/gtk4-layer-shell-0.pc"
echo "MELANIA_MINT_LAYER_SHELL_READY=$PREFIX"
echo "UPSTREAM_GTK4_LAYER_SHELL_SHA=$UPSTREAM_COMMIT"
