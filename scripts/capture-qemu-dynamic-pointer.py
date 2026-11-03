#!/usr/bin/env python3
"""Prove QMP CRTC pixels changed only via real mouse drag after xdg shrink."""
from __future__ import annotations
import argparse
import json
import re
import socket
import sys
import time
from pathlib import Path

W, H = 1280, 800
MARKER = "MELANIA_RUST_CONTINUOUS_DYNAMIC_POINTER_FRAME_VISIBLE=13"
EXIT = "MELANIA_QEMU_NATIVE_GUEST_EXIT="


def capture(qmp: Path, path: Path) -> None:
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.settimeout(6)
        sock.connect(str(qmp))
        stream = sock.makefile("rwb", buffering=0)
        if "QMP" not in json.loads(stream.readline()):
            raise RuntimeError("QMP greeting missing")
        for command in (
            {"execute": "qmp_capabilities"},
            {"execute": "screendump", "arguments": {
                "filename": str(path), "format": "ppm"}},
        ):
            stream.write((json.dumps(command) + "\n").encode())
            for _ in range(50):
                reply = json.loads(stream.readline())
                if "error" in reply:
                    raise RuntimeError("QMP screenshot rejected: " + str(reply["error"]))
                if "return" in reply:
                    break
            else:
                raise RuntimeError("QMP screenshot reply missing")
    if not path.is_file():
        raise RuntimeError("actual virtual GPU screenshot missing")


def color(index: int) -> tuple[int, int, int]:
    return (30 + index * 17 % 200, 40 + index * 31 % 190,
            25 + index * 43 % 210)


def blended_b(index: int) -> tuple[int, int, int]:
    # Real wl_shm B is premultiplied ARGB at opacity 128/255.
    return tuple(min(255, (c * 128 + 127) // 255 + (bg * 127 + 127) // 255)
                 for c, bg in zip(color(index), (27, 33, 58)))


def verify(data: bytes) -> dict[str, tuple[int, int, int]]:
    header = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", data)
    if not header or tuple(map(int, header.groups())) != (W, H):
        raise ValueError("wrong QEMU virtual GPU output dimensions")
    offset = header.end()
    if len(data) != offset + W * H * 3:
        raise ValueError("truncated or overlong QMP framebuffer")
    def rgb(x: int, y: int) -> tuple[int, int, int]:
        start = offset + 3 * (y * W + x)
        return tuple(data[start:start+3])
    # Drag from (760,200) to (850,280). B moves from (650,190)
    # to approximately (740,270); old pixels must disappear.
    expected = {
        "background": ((40, 40), (27, 33, 58)),
        "A_retained_frame11": ((200, 200), color(11)),
        "B_old_pixels_cleared": ((700, 250), (27, 33, 58)),
        "B_new_left": ((820, 380), blended_b(12)),
        "B_new_right": ((930, 420), blended_b(12)),
        "B_outside_new_edge": ((975, 415), (27, 33, 58)),
    }
    found = {}
    for label, (point, want) in expected.items():
        got = rgb(*point)
        found[label] = got
        if any(abs(a-b) > 18 for a, b in zip(got, want)):
            raise ValueError(f"{label}: actual {got} != expected {want}")
    return found


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--serial", type=Path, required=True)
    p.add_argument("--qmp", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--timeout", type=float, default=950)
    args = p.parse_args()
    deadline = time.monotonic() + args.timeout
    while time.monotonic() < deadline:
        log = args.serial.read_text(errors="replace") if args.serial.exists() else ""
        if MARKER in log.splitlines():
            break
        if EXIT in log:
            raise RuntimeError("guest exited before mouse-driven KMS")
        time.sleep(.06)
    else:
        raise TimeoutError("QMP tablet frame13 never reached virtual CRTC")
    last_error = "no screenshot"
    for _ in range(10):
        try:
            capture(args.qmp, args.output)
            pixels = verify(args.output.read_bytes())
            print(f"MELANIA_QEMU_DYNAMIC_POINTER_FRAME13_PIXELS=true {pixels}", flush=True)
            return 0
        except (OSError, RuntimeError, ValueError, EOFError) as exc:
            last_error = str(exc)
            time.sleep(.14)
    raise RuntimeError("real KMS drag screenshot verification failed: " + last_error)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError, TimeoutError, EOFError) as exc:
        print("MELANIA_QEMU_DYNAMIC_POINTER_FRAME13_PIXELS=false: " + str(exc),
              file=sys.stderr)
        raise SystemExit(1)
