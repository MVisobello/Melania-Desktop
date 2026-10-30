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


def close_button_widths(path: Path) -> list[tuple[int, int]]:
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
        dimensions = []
        for group in groups:
            if len(group) < 16:
                continue
            active_rows = []
            for y in range(min(48, height), min(140, height)):
                red_count = 0
                for x in group:
                    r, g, b = pixels[x, y]
                    if r > 135 and r > g * 1.4 and r > b * 1.35 and g < 175:
                        red_count += 1
                if red_count >= 5:
                    active_rows.append(y)
            if active_rows:
                dimensions.append((len(group), max(active_rows)-min(active_rows)+1))
        return sorted(dimensions, reverse=True)


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: check_shared_window_chrome.py <real-desktop.png>")
    buttons = close_button_widths(Path(sys.argv[1]))
    assert len(buttons) == 2, f"Expected both Files and Settings close buttons: {buttons}"
    widths = [w for w, _ in buttons]
    heights = [h for _, h in buttons]
    assert all(w >= 28 for w in widths), f"Close buttons too narrow: {buttons}"
    assert abs(widths[0]-widths[1]) <= 5, f"Close widths differ: {buttons}"
    assert all(h <= 33 for h in heights), f"Close buttons still fill titlebar height: {buttons}"
    assert abs(heights[0]-heights[1]) <= 5, f"Close heights differ: {buttons}"
    print(f"PASS: actual front/back Win7 close buttons match, widths={widths}, heights={heights}")


if __name__ == "__main__":
    main()
