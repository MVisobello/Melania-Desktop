#!/usr/bin/env python3
"""Stage47: prove the seventh and twelfth REAL KMS frames have correct QMP pixels.

Only a running sacrificial QEMU guest with an exposed private QMP socket is
used. The client sends twelve distinct real Wayland wl_shm commits; snapshots
must be captured while their respective DRM framebuffer remains active.
"""
from __future__ import annotations
import argparse
import json
import re
import socket
import sys
import time
from pathlib import Path

W, H = 1280, 800
EXIT = "MELANIA_QEMU_NATIVE_GUEST_EXIT="


def screendump(qmp: Path, output: Path) -> None:
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.settimeout(6)
        sock.connect(str(qmp))
        io = sock.makefile("rwb", buffering=0)
        if "QMP" not in json.loads(io.readline()):
            raise RuntimeError("QMP handshake not received")
        for command in (
            {"execute": "qmp_capabilities"},
            {"execute": "screendump", "arguments": {"filename": str(output), "format": "ppm"}},
        ):
            io.write((json.dumps(command) + "\n").encode())
            for _ in range(50):
                response = json.loads(io.readline())
                if "error" in response:
                    raise RuntimeError("QMP rejected screendump: " + str(response["error"]))
                if "return" in response:
                    break
            else:
                raise RuntimeError("QMP screenshot never completed")
    if not output.is_file():
        raise RuntimeError("QMP did not write screenshot")


def verify(data: bytes, frame: int) -> dict[str, tuple[int, int, int]]:
    if frame not in (7, 12):
        raise ValueError("only two independently proved frames are accepted")
    match = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", data)
    if not match or tuple(map(int, match.groups())) != (W, H):
        raise ValueError("unexpected QEMU virtual GPU resolution")
    offset = match.end()
    if len(data) != offset + W * H * 3:
        raise ValueError("truncated real virtual GPU screenshot")
    def rgb(x: int, y: int) -> tuple[int, int, int]:
        start = offset + (y * W + x) * 3
        return tuple(data[start:start + 3])
    # Match the actual formula in the QEMU-only twelve-frame Wayland client.
    red = 30 + (frame * 17) % 200
    green = 40 + (frame * 31) % 190
    blue = 25 + (frame * 43) % 210
    expected = {
        "background": ((40, 40), (27, 33, 58)),
        "window_border": ((157, 120), (65, 171, 216)),
        "real_client_pixels": ((210, 170), (red, green, blue)),
    }
    found = {}
    for name, (position, color) in expected.items():
        actual = rgb(*position)
        found[name] = actual
        if any(abs(got - want) > 18 for got, want in zip(actual, color)):
            raise ValueError(f"frame {frame} {name}: {actual} != {color}")
    return found


def await_marker(serial: Path, frame: int, deadline: float) -> None:
    marker = f"MELANIA_RUST_CONTINUOUS_FRAME_VISIBLE={frame}"
    while time.monotonic() < deadline:
        lines = serial.read_text(errors="replace").splitlines() if serial.exists() else []
        if marker in lines:
            return
        if any(line.startswith(EXIT) for line in lines):
            raise RuntimeError(f"VM exited before continuous frame {frame}")
        time.sleep(0.055)
    raise TimeoutError(f"continuous KMS frame {frame} was never active")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--serial", type=Path, required=True)
    parser.add_argument("--qmp", type=Path, required=True)
    parser.add_argument("--seventh", type=Path, required=True)
    parser.add_argument("--twelfth", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=950)
    args = parser.parse_args()
    deadline = time.monotonic() + args.timeout
    for frame, output in ((7, args.seventh), (12, args.twelfth)):
        await_marker(args.serial, frame, deadline)
        last_error = "screenshot not attempted"
        for _ in range(8):
            try:
                screendump(args.qmp, output)
                matched = verify(output.read_bytes(), frame)
                print(f"MELANIA_QEMU_CONTINUOUS_FRAME{frame}_PIXELS_VERIFIED=true {matched}",
                      flush=True)
                break
            except (OSError, ValueError, RuntimeError, EOFError) as error:
                last_error = str(error)
                time.sleep(0.12)
        else:
            raise RuntimeError(f"frame {frame} real QMP proof failed: {last_error}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError, TimeoutError, EOFError) as error:
        print("MELANIA_QEMU_CONTINUOUS_QMP_PROOF=false: " + str(error), file=sys.stderr)
        raise SystemExit(1)
