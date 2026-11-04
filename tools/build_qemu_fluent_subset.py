#!/usr/bin/env python3
"""Build a small *real* Melania Fluent freedesktop icon pack for the QEMU ISO.

Reuse the exact approved original SVG generator from the repository. Do not
copy artwork from Flat Remix or invent PNG images for the screenshot.
This subset is a test fixture, NOT the full distributable three-theme pack.
"""
from __future__ import annotations

import argparse
from pathlib import Path
from build_melania_fluent_themes import build, THEMES
from melania_fluent_artwork import CORE

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = (
    "apps/scalable/melania.svg",
    "apps/scalable/org.melaniadesktop.Files.svg",
    "apps/scalable/org.melaniadesktop.Settings.svg",
    "apps/24x24/system-file-manager.svg",
    "places/24x24/folder.svg",
    "status/24x24/network-wireless-signal-excellent-symbolic.svg",
)


def selected_paths() -> list[str]:
    paths = set()
    for _label, name, category in CORE:
        paths.add(f"{category}/24x24/{name}.svg")
        paths.add(f"{category}/scalable/{name}.svg")
    # The theme generator itself adds the Melania logo, exact application
    # aliases and every taskbar/sidebar symbolic name at 16/24/scalable.
    return sorted(paths)


def verify(root: Path) -> None:
    if not root.is_dir():
        raise FileNotFoundError("Melania Fluent QEMU icon tree absent")
    for name in THEMES:
        theme = root / name
        if not (theme / "index.theme").is_file():
            raise ValueError(f"Melania Fluent icon index missing: {name}")
        for key in REQUIRED:
            icon = theme / key
            if not icon.is_file():
                raise ValueError(f"Melania Fluent icon missing or symlink broken: {name}/{key}")
            # Fail if the relative link escapes the generated VM test asset.
            if not icon.resolve().is_relative_to(root.resolve()):
                raise ValueError(f"Melania Fluent icon unexpectedly points outside its ISO tree: {icon}")
    print("MELANIA_QEMU_FLUENT_ICON_SUBSET_VERIFIED=true", flush=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=ROOT / "target" / "melania-qemu-fluent-icons")
    parser.add_argument("--verify-only", action="store_true")
    args = parser.parse_args()
    output = args.output.resolve()
    if not args.verify_only:
        if output.exists():
            raise FileExistsError(f"Refusing to overwrite existing QEMU theme tree: {output}")
        build(output, selected_paths(), extra_sizes=False)
    verify(output)


if __name__ == "__main__":
    main()
