#!/usr/bin/env python3
"""Capture two *different* real GTK4 wl_shm taskbar frames from LIVE QEMU CRTC.

The GTK4 client itself changes its CSS only in the disposable VM experiment;
no synthetic pixel images or screenshots are accepted as evidence. The QMP
monitor is read while each indexed DRM framebuffer is on scanout.
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
    "melania_single_gtk_qmp", ROOT / "capture-qemu-real-gtk-panel.py")
assert spec is not None and spec.loader is not None
single = importlib.util.module_from_spec(spec)
spec.loader.exec_module(single)

PREFIX = "MELANIA_RUST_REAL_GTK_TWO_FRAME_VISIBLE="
EXIT = "MELANIA_QEMU_NATIVE_GUEST_EXIT="


def wait_for_visible(serial: Path, ordinal: int, deadline: float) -> None:
    needle = f"{PREFIX}{ordinal}"
    while time.monotonic() < deadline:
        lines = serial.read_text(errors="replace").splitlines() if serial.exists() else []
        if needle in lines:
            return
        if any(line.startswith(EXIT) for line in lines):
            raise RuntimeError(f"guest exited before the actual GTK4 frame {ordinal} reached KMS")
        time.sleep(0.065)
    raise TimeoutError(f"real GTK4 KMS frame {ordinal} not visible")


def read_rgb(ppm: Path) -> bytes:
    content = ppm.read_bytes()
    single.validate_ppm(content)
    m = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", content)
    if not m:
        raise ValueError("QMP screenshot missing original P6 header")
    return content[m.end():]


def compare_real_frames(first: bytes, second: bytes) -> int:
    width, height, top = single.WIDTH, single.HEIGHT, single.PANEL_TOP
    if len(first) != width * height * 3 or len(second) != len(first):
        raise ValueError("two real GTK4 QMP captures have incorrect payload lengths")
    outside = top * width * 3
    if first[:outside] != second[:outside]:
        raise ValueError("non-panel QEMU scene changed between GTK4 frames")
    # The actual GTK CSS background changes after the first KMS ACK; compare
    # full RGB pixels in the panel, not fake marker strings or two timestamps.
    changed = sum(a != b for a, b in zip(first[outside:], second[outside:]))
    if changed < 2400:
        raise ValueError(f"GTK4 second frame did not repaint actual taskbar pixels: {changed}")
    return changed


def run(args: argparse.Namespace) -> None:
    deadline = time.monotonic() + args.timeout
    frames = []
    for ordinal, destination in enumerate((args.first, args.second), start=1):
        wait_for_visible(args.serial, ordinal, deadline)
        error = "frame not captured"
        for _ in range(12):
            try:
                single.capture(args.qmp, destination)
                pixels = read_rgb(destination)
                frames.append(pixels)
                print(f"MELANIA_QEMU_GTK4_FRAME{ordinal}_QMP_REAL=true", flush=True)
                break
            except (OSError, RuntimeError, ValueError, EOFError) as exc:
                error = str(exc)
                time.sleep(0.08)
        else:
            raise RuntimeError(f"real GTK4 frame {ordinal} QMP failed: {error}")
    changed = compare_real_frames(frames[0], frames[1])
    print(f"MELANIA_QEMU_GTK4_TWO_DISTINCT_REAL_FRAMES=true changed_rgb_bytes={changed}", flush=True)


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--serial", type=Path, required=True)
    p.add_argument("--qmp", type=Path, required=True)
    p.add_argument("--first", type=Path, required=True)
    p.add_argument("--second", type=Path, required=True)
    p.add_argument("--timeout", type=float, default=950)
    run(p.parse_args())


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, OSError, ValueError, TimeoutError) as exc:
        print(f"MELANIA_QEMU_GTK4_TWO_DISTINCT_REAL_FRAMES=false: {exc}", file=sys.stderr)
        raise SystemExit(1)
