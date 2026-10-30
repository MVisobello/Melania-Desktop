#!/usr/bin/env python3
"""Audit visual diversity and filesystem integrity of Melania icon variants.

Run after build_melania_icon_variants.py. Never reads Flat Remix artwork.
Returns a nonzero exit code for broken links or missing required themes.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import xml.etree.ElementTree as ET

THEMES = ("Melania-Light", "Melania-Dark", "Melania-Light-on-Dark-Panel")
CONTEXTS = ("actions", "apps", "animations", "categories", "devices",
            "emblems", "mimetypes", "panel", "places", "status")


def analyze(root: Path) -> dict:
    results = {}
    integrity_errors = []
    for name in THEMES:
        theme = root / name
        if not (theme / "index.theme").is_file():
            integrity_errors.append(f"{name}: index.theme missing")
            continue
        paths = sorted(theme.rglob("*.svg"))
        # rglob does not traverse linked directory trees. Use the expected
        # manifests to check icon names below, without traversing symlink loops.
        observed = set()
        category_counts = Counter()
        design_hashes = defaultdict(set)
        provenance = Counter()
        for directory in CONTEXTS:
            folder = theme / directory
            if not folder.exists():
                integrity_errors.append(f"{name}/{directory}: missing")
                continue
            for icon in folder.rglob("*.svg"):
                relative = icon.relative_to(theme).as_posix()
                observed.add(relative)
                category_counts[directory] += 1
                try:
                    content = icon.read_bytes()
                    ET.fromstring(content)
                    design_hashes[directory].add(hashlib.sha256(content).hexdigest())
                    if b"data:image/png;base64" in content:
                        provenance["embedded Melania PNG"] += 1
                    elif b"font-family=" in content or b"Original Melania" in content:
                        provenance["generated vector"] += 1
                    else:
                        provenance["SVG vector (origin undetermined)"] += 1
                except (OSError, ET.ParseError) as exc:
                    integrity_errors.append(f"{name}/{relative}: {exc}")
        # Check every symbolic link, including those in non-traversed folders.
        # The full manifest check below catches broken links within linked dirs.
        for link in theme.rglob("*"):
            if link.is_symlink() and not link.exists():
                integrity_errors.append(f"{name}/{link.relative_to(theme)}: broken link")
        results[name] = {
            "directly_walked_svg": len(observed),
            "directly_walked_by_context": dict(sorted(category_counts.items())),
            "unique_artworks_by_context": {
                k: len(v) for k, v in sorted(design_hashes.items())},
            "observed_provenance": dict(sorted(provenance.items())),
        }

    # Reference inventories are names only. This is a compatibility audit;
    # it does not compare or reproduce Flat Remix artwork.
    manifest_dir = root.resolve().parents[1] / "branding" / "coverage"
    if not manifest_dir.exists():
        # Support --output pointing to an arbitrary /tmp directory.
        manifest_dir = Path(__file__).resolve().parents[1] / "branding" / "coverage"
    dark = manifest_dir / "reference-dark-paths.txt"
    light = manifest_dir / "reference-light-paths.txt"
    if dark.is_file() and light.is_file():
        required = set(dark.read_text(encoding="utf8").splitlines()) | set(
            light.read_text(encoding="utf8").splitlines())
        required = {p for p in required if p and not p.startswith("#")}
        for name in THEMES:
            if not (root / name / "index.theme").exists():
                continue
            missing = [p for p in sorted(required) if not (root / name / p).is_file()]
            results[name]["reference_identifiers_checked"] = len(required)
            results[name]["unresolved_identifiers"] = len(missing)
            if missing:
                integrity_errors.append(f"{name}: {len(missing)} missing, e.g. {missing[:5]}")
    return {
        "themes": results,
        "integrity_ok": not integrity_errors,
        "errors": integrity_errors[:50],
        "note": ("SVG identifiers and link aliases are not unique drawings. "
                 "Counts reflect technical coverage, not artistic completion.")
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path,
                        default=Path(__file__).resolve().parents[1] / "dist" / "melania-icon-themes")
    parser.add_argument("--report", type=Path, default=None)
    args = parser.parse_args()
    report = analyze(args.output.resolve())
    data = json.dumps(report, indent=2, ensure_ascii=False)
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(data + "\n", encoding="utf8")
    print(data)
    return 0 if report["integrity_ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
