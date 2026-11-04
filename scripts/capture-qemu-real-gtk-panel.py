#!/usr/bin/env python3
"""Verify an actual QEMU KMS screendump of the genuine GTK4 Melania panel.

No mock colors or saved screenshots can satisfy this evidence: capture happens
only after the Rust guest DRM receiver reports its new CRTC framebuffer.
"""
from __future__ import annotations

import argparse
import json
import re
import socket
import sys
import time
from pathlib import Path

WIDTH, HEIGHT = 1280, 800
# The real GTK4 content has a 60px CSS natural minimum, although its initial
# layer-shell configure requests 52px. Preserve the running app's appearance.
PANEL_HEIGHT = 60
PANEL_TOP = HEIGHT - PANEL_HEIGHT
BACKGROUND = (27, 33, 58)
FRAME_MARKER = "MELANIA_RUST_REAL_GTK_PANEL_FRAME_VISIBLE=1"
EXIT_MARKER = "MELANIA_QEMU_NATIVE_GUEST_EXIT="


def validate_ppm(payload: bytes) -> tuple[int, int]:
    header = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", payload)
    if not header or tuple(map(int, header.groups())) != (WIDTH, HEIGHT):
        raise ValueError("QMP framebuffer must be the real 1280x800 VirtIO scanout")
    offset = header.end()
    if len(payload) != offset + WIDTH * HEIGHT * 3:
        raise ValueError("truncated KMS framebuffer screendump")

    def rgb(x: int, y: int) -> tuple[int, int, int]:
        start = offset + (y * WIDTH + x) * 3
        return tuple(payload[start:start + 3])

    # Above the genuine 60px GTK buffer the scene must still have the exact
    # QEMU background. Reject invented full-screen images and leaked pixels.
    for y in (100, 550, 700, PANEL_TOP - 1):
        for x in (0, 101, 440, 800, 1177, 1279):
            actual = rgb(x, y)
            if max(abs(a - b) for a, b in zip(actual, BACKGROUND)) > 10:
                raise ValueError(f"unexpected scene pixel outside GTK taskbar at {x},{y}: {actual}")

    changed = 0
    tones: set[tuple[int, int, int]] = set()
    for y in range(PANEL_TOP, HEIGHT):
        for x in range(WIDTH):
            actual = rgb(x, y)
            if max(abs(a - b) for a, b in zip(actual, BACKGROUND)) >= 8:
                changed += 1
            if x % 2 == 0 and y % 2 == 0:
                tones.add(tuple(c // 12 for c in actual))
    if changed < 500:
        raise ValueError(f"real GTK taskbar does not visibly differ from background: {changed} pixels")
    if len(tones) < 4:
        raise ValueError(f"GTK taskbar lacks expected graphical variation: {len(tones)} tones")
    return changed, len(tones)


def capture(qmp: Path, destination: Path) -> None:
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as conn:
        conn.settimeout(6)
        conn.connect(str(qmp))
        wire = conn.makefile("rwb", buffering=0)
        if "QMP" not in json.loads(wire.readline()):
            raise RuntimeError("QMP greeting missing")
        for request in (
            {"execute": "qmp_capabilities"},
            {"execute": "screendump", "arguments": {"filename": str(destination), "format": "ppm"}},
        ):
            wire.write((json.dumps(request) + "\n").encode())
            for _ in range(50):
                result = json.loads(wire.readline())
                if "error" in result:
                    raise RuntimeError(f"QMP screendump failed: {result['error']}")
                if "return" in result:
                    break
            else:
                raise RuntimeError("QMP screendump reply missing")
    if not destination.is_file():
        raise RuntimeError("actual QMP framebuffer screenshot absent")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--serial", type=Path, required=True)
    parser.add_argument("--qmp", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=950)
    args = parser.parse_args()
    deadline = time.monotonic() + args.timeout
    while time.monotonic() < deadline:
        log = args.serial.read_text(errors="replace") if args.serial.exists() else ""
        if FRAME_MARKER in log.splitlines():
            break
        if EXIT_MARKER in log:
            raise RuntimeError("guest exited before native GTK4 frame reached DRM/KMS")
        time.sleep(0.08)
    else:
        raise TimeoutError("genuine GTK4 KMS frame never became visible")

    last_error = "not captured"
    for _ in range(12):
        try:
            capture(args.qmp, args.output)
            changed, tones = validate_ppm(args.output.read_bytes())
            print(f"MELANIA_QEMU_GTK4_PANEL_PIXELS_FROM_KMS=true changed={changed} tones={tones}", flush=True)
            return 0
        except (OSError, ValueError, RuntimeError, EOFError) as exc:
            last_error = str(exc)
            time.sleep(0.10)
    raise RuntimeError("real GTK4 QMP scanout verification failed: " + last_error)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, ValueError, TimeoutError, EOFError) as exc:
        print(f"MELANIA_QEMU_GTK4_PANEL_PIXELS_FROM_KMS=false: {exc}", file=sys.stderr)
        raise SystemExit(1)
