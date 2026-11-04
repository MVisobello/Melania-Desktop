#!/usr/bin/env python3
"""Losslessly encode the live QMP GTK4 framebuffer PPM into a reviewable PNG.

This does not generate images. Source bytes must be the real KMS screendump
already validated by capture-qemu-real-gtk-panel.py; reject missing, malformed
or unexpected captures. Uses only Python's standard library.
"""
from __future__ import annotations

import argparse
import struct
import sys
import zlib
from pathlib import Path

WIDTH = 1280
HEIGHT = 800
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def read_ppm(path: Path) -> bytes:
    data = path.read_bytes()
    lines = data.split(b"\n", 3)
    if len(lines) != 4 or lines[:3] != [b"P6", b"1280 800", b"255"]:
        raise ValueError("not an unmodified 1280x800 QEMU P6 scanout")
    pixels = lines[3]
    if len(pixels) != WIDTH * HEIGHT * 3:
        raise ValueError("QMP framebuffer payload has an incorrect length")
    return pixels


def chunk(tag: bytes, body: bytes) -> bytes:
    return (
        struct.pack(">I", len(body))
        + tag
        + body
        + struct.pack(">I", zlib.crc32(body, zlib.crc32(tag)) & 0xFFFFFFFF)
    )


def encode_rgb_png(pixels: bytes) -> bytes:
    if len(pixels) != WIDTH * HEIGHT * 3:
        raise ValueError("RGB image must contain the exact QEMU framebuffer")
    row = WIDTH * 3
    # PNG filter 0 preserves each 8-bit RGB pixel unchanged. No mock buffer,
    # color correction or image interpolation is performed.
    filtered = bytearray((row + 1) * HEIGHT)
    for y in range(HEIGHT):
        start = y * (row + 1)
        filtered[start] = 0
        filtered[start + 1:start + 1 + row] = pixels[y * row:(y + 1) * row]
    ihdr = struct.pack(">IIBBBBB", WIDTH, HEIGHT, 8, 2, 0, 0, 0)
    return (
        PNG_SIGNATURE
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(filtered, level=6))
        + chunk(b"IEND", b"")
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ppm", type=Path, required=True)
    parser.add_argument("--png", type=Path, required=True)
    args = parser.parse_args()
    pixels = read_ppm(args.ppm)
    if args.png.resolve() == args.ppm.resolve():
        raise ValueError("PNG must not replace its original PPM evidence")
    args.png.write_bytes(encode_rgb_png(pixels))
    print("MELANIA_QEMU_REAL_GTK_PNG_EXPORTED=true", flush=True)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError) as error:
        print(f"MELANIA_QEMU_REAL_GTK_PNG_EXPORTED=false: {error}", file=sys.stderr)
        raise SystemExit(1)
