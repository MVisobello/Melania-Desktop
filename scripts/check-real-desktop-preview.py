#!/usr/bin/env python3
"""Publish a real Melania nested-desktop screenshot with verifiable provenance.

Input images are captured from the running Smithay compositor hosted by
Xvfb/X11, with *real* GTK Files, Settings, desktop and panel processes.
Never generate/draw/replace any UI pixels. The output PNG is an unmodified
byte-for-byte copy of the source screenshot.

This is NOT proof of a native DRM/KMS graphical session.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil

from PIL import Image, ImageChops

FILES = re.compile(r"(?:org\.melaniadesktop\.files|melania[\s._-]*files)", re.I)
SETTINGS = re.compile(r"(?:org\.melaniadesktop\.settings|impostazioni|settings)", re.I)


def verify(screenshot: Path, search: Path, windows_log: Path) -> dict:
    if not screenshot.is_file() or not search.is_file():
        raise ValueError("both genuine compositor screenshots are required")
    window_text = windows_log.read_text(encoding="utf-8")
    if not FILES.search(window_text) or not SETTINGS.search(window_text):
        raise ValueError("Files and Settings must be simultaneously mapped")
    with Image.open(screenshot) as original, Image.open(search) as typing:
        if original.format != "PNG" or typing.format != "PNG":
            raise ValueError("unmodified PNG captures required")
        if original.size != typing.size or original.width < 1024 or original.height < 600:
            raise ValueError("screen dimensions disagree or are too small")
        a, b = original.convert("RGB"), typing.convert("RGB")
        sample = a.resize((128, 80))
        unique = len(set(sample.getdata()))
        bright = sum(1 for r, g, bl in sample.getdata() if min(r, g, bl) > 185)
        if unique <= 8 or bright <= 300:
            raise ValueError(f"blank/unpainted UI: colors={unique}, bright={bright}")
        # Both are true screenshots, taken before and after typing into the
        # already-visible Start menu. A byte-identical screenshot is rejected.
        changes = ImageChops.difference(a, b)
        if changes.getbbox() is None:
            raise ValueError("Start search screenshot identical to initial screenshot")
        return {
            "capture_type": "real_nested_wayland_xvfb",
            "native_kms_desktop": False,
            "synthetic_ui_pixels": False,
            "image_size": list(original.size),
            "distinct_sample_colors": unique,
            "bright_sample_pixels": bright,
            "files_window_mapped": True,
            "settings_window_mapped": True,
            "start_search_visual_change": True,
            "source_sha256": hashlib.sha256(screenshot.read_bytes()).hexdigest(),
            "search_sha256": hashlib.sha256(search.read_bytes()).hexdigest(),
        }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--screenshot", required=True, type=Path)
    parser.add_argument("--search", required=True, type=Path)
    parser.add_argument("--windows", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    args = parser.parse_args()
    evidence = verify(args.screenshot, args.search, args.windows)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.manifest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(args.screenshot, args.output)
    if hashlib.sha256(args.output.read_bytes()).hexdigest() != evidence["source_sha256"]:
        raise RuntimeError("preview is not byte-for-byte identical to captured desktop")
    args.manifest.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
    print("MELANIA_REAL_NESTED_DESKTOP_PREVIEW=PASS")
    print("MELANIA_NATIVE_KMS_DESKTOP_READY=false")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
