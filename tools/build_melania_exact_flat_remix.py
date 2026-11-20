#!/usr/bin/env python3
"""Rebuild Melania Fluent icons with exact Flat Remix Yellow filesystem layout.

Source SVG content is deliberately never read. Only entry names, link targets
and index.theme metadata are consumed from the user-supplied tar.xz.
Existing Melania Fluent artwork modules supply wholly separate SVG graphics.
"""
from __future__ import annotations

import argparse
from collections import Counter
import json
import os
from pathlib import Path, PurePosixPath
import re
import tarfile
import xml.etree.ElementTree as ET

from melania_fluent_artwork import artwork as generic_artwork, REVERSE
from melania_fluent_reference import artwork as reference_artwork
from melania_fluent_settings_icons import artwork as settings_artwork
from melania_shell_symbolic_icons import artwork as shell_artwork

ROOT = Path(__file__).resolve().parents[1]
MAP = {
    "Flat-Remix-Yellow-Dark": "Melania-Dark",
    "Flat-Remix-Yellow-Light": "Melania-Light",
    "Flat-Remix-Yellow-Light-darkPanel": "Melania-Light-darkPanel",
}
CONTEXTS = {
    "actions", "animations", "apps", "categories", "devices", "emblems",
    "mimetypes", "panel", "places", "status"
}
LOGO_NAMES = {"melania", "start-here", "distributor-logo", "org.melaniadesktop.Preview"}


def rename(text: str) -> str:
    for upstream, melania in MAP.items():
        text = text.replace(upstream, melania)
    return text


def size_of(path: PurePosixPath) -> int:
    for segment in path.parts[:-1]:
        match = re.fullmatch(r"(\d+)(?:x\d+)?", segment)
        if match:
            return int(match.group(1))
    return 24 if path.parts[0] in ("panel", "status", "animations") else 96


def draw_icon(path: PurePosixPath, name: str, theme: str) -> tuple[str, str]:
    category = path.parts[0]
    if category not in CONTEXTS:
        raise ValueError(f"Invalid freedesktop context: {path}")
    size = size_of(path)
    contrast = (
        category in ("actions", "emblems", "panel", "status", "animations")
        or "symbolic" in path.parts or name.endswith("-symbolic")
    )
    variant = "dark" if theme == "Melania-Dark" else "light"
    if not contrast:
        variant = "dark"  # full-color Melania illustrations are theme-stable
    if category == "apps" and name in LOGO_NAMES:
        svg = (ROOT / "branding/icons/melania-symbol.svg").read_text(encoding="utf8")
        family = "brand-logo"
    else:
        svg = None
        family = "generated"
        for label, provider in (
            ("shell-symbol", shell_artwork),
            ("approved-artwork", reference_artwork),
            ("settings", settings_artwork),
            ("semantic-artwork", generic_artwork),
        ):
            svg = provider(name, category, variant, size)
            if svg is not None:
                family = label
                break
        if not svg:
            raise AssertionError(f"No Melania SVG generated for {path}")
    ET.fromstring(svg)
    if "<script" in svg.lower() or "file://" in svg.lower():
        raise ValueError(f"Unsafe icon content: {path}")
    return svg, family


def build(reference: Path, target: Path) -> dict:
    if target.exists() and any(target.iterdir()):
        raise FileExistsError(f"Output must be empty: {target}")
    target.mkdir(parents=True, exist_ok=True)
    stats: dict[str, Counter] = {value: Counter() for value in MAP.values()}
    visual = Counter()
    ref_inventory: set[tuple[str, str, str]] = set()
    expected_links: list[Path] = []
    with tarfile.open(reference, "r:*") as archive:
        for entry in archive:
            path = PurePosixPath(entry.name.removeprefix("./"))
            if path.is_absolute() or ".." in path.parts or not path.parts:
                raise ValueError(f"Unsafe reference path: {entry.name}")
            upstream = path.parts[0]
            if upstream not in MAP:
                raise ValueError(f"Unexpected theme: {upstream}")
            theme = MAP[upstream]
            relative = PurePosixPath(*path.parts[1:])
            if not relative.parts:
                continue
            destination = target / theme / relative
            if destination.exists() or destination.is_symlink():
                raise ValueError(f"Duplicate archive entry: {entry.name}")
            destination.parent.mkdir(parents=True, exist_ok=True)
            kind = (
                "directory" if entry.isdir()
                else "symlink" if entry.issym()
                else "file" if entry.isfile()
                else "unsupported"
            )
            ref_inventory.add((theme, str(relative), kind))
            stats[theme][kind] += 1
            if kind == "directory":
                destination.mkdir(exist_ok=True)
            elif kind == "symlink":
                destination.symlink_to(rename(entry.linkname))
                expected_links.append(destination)
            elif kind == "file" and relative.name == "index.theme":
                # freedesktop theme metadata, not source icon artwork
                metadata = archive.extractfile(entry).read().decode("utf8")
                metadata = rename(metadata).replace("Flat Remix", "Melania Fluent")
                destination.write_text(metadata, encoding="utf8")
            elif kind == "file" and relative.suffix == ".svg":
                # NEVER read archive.extractfile(entry) for SVG source content.
                content, family = draw_icon(relative, relative.stem, theme)
                destination.write_text(content, encoding="utf8")
                visual[family] += 1
                stats[theme]["new-svg"] += 1
            else:
                raise ValueError(f"Unexpected archive entry: {entry.name} ({kind})")

    broken = [str(link) for link in expected_links if not link.exists()]
    if broken:
        raise AssertionError(f"Broken reference aliases ({len(broken)}): {broken[:8]}")
    for theme in MAP.values():
        index = target / theme / "index.theme"
        if not index.is_file():
            raise AssertionError(f"Missing freedesktop theme index: {theme}")
    report = {
        "reference": reference.name,
        "file_tree_exact": True,
        "broken_links": len(broken),
        "themes": {theme: dict(stats[theme]) for theme in MAP.values()},
        "svg_artwork_providers": dict(visual),
        "note": "Path parity is complete; some uncommon application artwork remains generated and needs visual review.",
    }
    (target / "BUILD-REPORT-EXACT.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf8"
    )
    return report


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", type=Path, required=True,
                        help="Original 04-Flat-Remix-Yellow-20251119.tar.xz")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(build(args.reference.resolve(), args.output.resolve()),
                     ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
