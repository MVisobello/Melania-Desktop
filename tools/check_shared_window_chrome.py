#!/usr/bin/env python3
"""Verify real desktop screenshot has equally sized Win7 close buttons.

In the nested GTK smoke test, Melania Settings is in front of Files, so
both titlebars are visible at the top of the screenshot. Detect the two red
close-button surfaces. This catches the former native-GTK-vs-custom width
mismatch even if source-code contract tests still pass.
"""
from __future__ import annotations
import sys
from pathlib import Path
from PIL import Image


def close_button_widths(path: Path) -> list[int]:
    with Image.open(path) as original:
        im = original.convert("RGB")
        width, height = im.size
        pixels = im.load()
        active_columns: list[int] = []
        for x in range(width):
            reds = 0
            for y in range(min(48, height), min(140, height)):
                r, g, b = pixels[x, y]
                # The branded crimson Aero close button, avoiding wallpaper.
                if r > 135 and r > g * 1.4 and r > b * 1.35 and g < 175:
                    reds += 1
            if reds >= 5:
                active_columns.append(x)
        groups: list[list[int]] = []
        for x in active_columns:
            if not groups or x > groups[-1][-1] + 1:
                groups.append([])
            groups[-1].append(x)
        return sorted((len(g) for g in groups if len(g) >= 16), reverse=True)


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: check_shared_window_chrome.py <real-desktop.png>")
    widths = close_button_widths(Path(sys.argv[1]))
    assert len(widths) == 2, f"Expected both Files and Settings red close buttons: {widths}"
    assert all(w >= 35 for w in widths), f"Close buttons too narrow: {widths}"
    assert abs(widths[0] - widths[1]) <= 5, (
        f"Win7 close buttons mismatch between front and background windows: {widths}"
    )
    print(f"PASS: real Files and Settings Win7 close buttons match, widths={widths}")


if __name__ == "__main__":
    main()
