#!/usr/bin/env python3
"""Validate real GTK4 Start above the real taskbar on the QEMU CRTC.

Stage86 piggybacks on Stage85's live first/two-frame watcher. The third image
must come from QMP while the Rust DRM receiver holds its verified FB_ID.
Do not create synthetic screenshots or approve an unchanged taskbar.
"""
from __future__ import annotations

import argparse
import importlib.util
import re
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location(
    "melania_live_qmp_gtk", ROOT / "capture-qemu-real-gtk-panel.py")
assert spec is not None and spec.loader is not None
qmp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qmp)

START_VISIBLE = "MELANIA_RUST_REAL_GTK_START_FRAME_VISIBLE=3"
EXIT = "MELANIA_QEMU_NATIVE_GUEST_EXIT="
# Melania's real zwlr_layer_shell_v1 melania-start window is configured for
# x=8, y=229, width=510, height=565 on the 1280x800 QEMU monitor.
START_X0, START_X1 = 8, 518
START_Y0, START_Y1 = 229, 740


def pixels(path: Path) -> bytes:
    raw = path.read_bytes()
    header = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", raw)
    if not header or tuple(map(int, header.groups())) != (1280, 800):
        raise ValueError("Start screenshot is not a real-sized QEMU PPM")
    image = raw[header.end():]
    if len(image) != 1280 * 800 * 3:
        raise ValueError("Start screenshot has truncated framebuffer payload")
    return image


def validate_start_open(previous: bytes, opened: bytes) -> int:
    width, height = qmp.WIDTH, qmp.HEIGHT
    if len(previous) != len(opened) or len(opened) != width * height * 3:
        raise ValueError("QMP Start framebuffer lengths differ")
    # A real Start overlay changes ONLY the allowed left-bottom area. The
    # wallpaper/empty desktop and opposite side of screen must stay fixed.
    for y in (0, 125, 210, 310, 555, 700):
        for x in (600, 800, 1050, 1279):
            pos = (y * width + x) * 3
            if max(abs(a-b) for a,b in zip(previous[pos:pos+3],
                                           opened[pos:pos+3])) > 8:
                raise ValueError("QEMU background changed outside GTK Start bounds")
    # The second frame's taskbar is already validated by Stage85. Only count
    # changes ABOVE the taskbar, where Stage85 cannot draw by design.
    changed = 0
    distinct = set()
    for y in range(START_Y0, START_Y1):
        for x in range(START_X0, START_X1):
            pos = (y * width + x) * 3
            old, new = previous[pos:pos+3], opened[pos:pos+3]
            if max(abs(int(a)-int(b)) for a,b in zip(old,new)) > 10:
                changed += 1
            if x % 12 == 0 and y % 12 == 0:
                distinct.add(tuple(channel // 16 for channel in new))
    if changed < 18000:
        raise ValueError(f"genuine GTK Start overlay did not appear: {changed} changed pixels")
    if len(distinct) < 7:
        raise ValueError("QEMU Start overlay lacks actual graphical content")
    return changed


def capture(args: argparse.Namespace) -> None:
    deadline = time.monotonic() + args.timeout
    while time.monotonic() < deadline:
        lines = args.serial.read_text(errors="replace").splitlines() if args.serial.exists() else []
        if START_VISIBLE in lines:
            break
        if any(line.startswith(EXIT) for line in lines):
            raise RuntimeError("guest exited before actual GTK Start image reached the CRTC")
        time.sleep(0.07)
    else:
        raise TimeoutError("third real GTK Start CRTC frame never appeared")
    # A screenshot from a previous run or a hardcoded PPM is not allowed:
    # read the real Stage85 screenshot, then QMP screendump this third FB_ID.
    first = pixels(args.second)
    last_error = "live QMP did not produce a Start screenshot"
    for _ in range(12):
        try:
            qmp.capture(args.qmp, args.output)
            changed = validate_start_open(first, pixels(args.output))
            print(f"MELANIA_QEMU_GTK_START_REAL_CRTC_PIXELS=true changed={changed}", flush=True)
            return
        except (OSError, ValueError, RuntimeError, EOFError) as error:
            last_error = str(error)
            time.sleep(0.08)
    raise RuntimeError("real GTK Start QMP capture failed: " + last_error)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--serial", type=Path, required=True)
    parser.add_argument("--qmp", type=Path, required=True)
    parser.add_argument("--second", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=950)
    capture(parser.parse_args())


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, TimeoutError) as error:
        print(f"MELANIA_QEMU_GTK_START_REAL_CRTC_PIXELS=false: {error}", file=sys.stderr)
        raise SystemExit(1)
