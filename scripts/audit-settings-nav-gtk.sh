#!/usr/bin/env bash
# Exercise every Settings sidebar destination in a REAL GTK4 process under Xvfb.
# This deliberately tests the running application, not just regexes in source.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p reports
export MELANIA_SETTINGS_AUDIT=1
log=reports/settings-nav-gtk-audit.log
: > "$log"

target/debug/melania-settings system > "$log" 2>&1 &
pid=$!
cleanup() { kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; }
trap cleanup EXIT

ready=false
for attempt in $(seq 1 60); do
  kill -0 "$pid" || { cat "$log"; echo "FAIL: Settings exited" >&2; exit 1; }
  if grep -q '^MELANIA_SETTINGS_PAGES_OK=25$' "$log" &&
     grep -q '^MELANIA_SETTINGS_ACTIVE_PAGE=system$' "$log"; then
    ready=true
    break
  fi
  sleep 0.2
done
if [ "$ready" != true ]; then cat "$log"; echo "FAIL: GTK page registry not ready" >&2; exit 1; fi

mapfile -t destinations < <(python3 - <<'PY'
import re
from pathlib import Path
source=Path("melania-settings/src/main.rs").read_text(encoding="utf-8")
registry=re.search(r"const SETTINGS_NAVIGATION:.*?= \[(.*?)\];",source,re.S)
assert registry, "Missing Settings navigation registry"
names=re.findall(r'^\s*\("[^"]+",\s*"([^"]+)",\s*"[^"]+"\),',registry.group(1),re.M)
assert len(names)==25 and len(set(names))==25, names
print("\n".join(names))
PY
)
selected=(system appearance applications xfce-panel xfce-desktop privacy)
for page in "${destinations[@]}"; do
  # GTK single-instance command-line activation must navigate the existing
  # window, not create a disconnected duplicate.
  target/debug/melania-settings "$page" > /dev/null 2>&1
  found=false
  for attempt in $(seq 1 40); do
    kill -0 "$pid" || { cat "$log"; echo "FAIL: Settings crashed on $page" >&2; exit 1; }
    current=$(grep '^MELANIA_SETTINGS_ACTIVE_PAGE=' "$log" | tail -n 1 || true)
    if [ "$current" = "MELANIA_SETTINGS_ACTIVE_PAGE=$page" ]; then
      found=true
      break
    fi
    sleep 0.15
  done
  if [ "$found" != true ]; then cat "$log"; echo "FAIL: navigation did not open $page" >&2; exit 1; fi
  for sample in "${selected[@]}"; do
    if [ "$page" = "$sample" ]; then
      sleep 0.5
      scrot -q 90 "reports/settings-audit-$page.png"
    fi
  done
  echo "PASS: $page"
done

python3 - <<'PY'
from pathlib import Path
from PIL import Image, ImageChops, ImageStat
samples = ["system","appearance","applications","xfce-panel","xfce-desktop","privacy"]
images = {}
for name in samples:
    path=Path(f"reports/settings-audit-{name}.png")
    with Image.open(path) as image:
        rgb=image.convert("RGB")
        assert rgb.size == (1280,800), (name,rgb.size)
        assert ImageStat.Stat(rgb).stddev[0] > 7, f"{name} is blank"
        images[name]=rgb.copy().crop((360,115,1160,710))
# Moving between pages must actually alter the visible content, not merely
# change a page ID while continuing to show the original system overview.
for previous, current in zip(samples,samples[1:]):
    delta=ImageStat.Stat(ImageChops.difference(images[previous],images[current])).mean
    assert sum(delta)/3 > 0.6, f"{previous} and {current} render identically"
print("PASS: all 25 sidebar destinations navigable; six distinct live GTK pages captured")
PY
