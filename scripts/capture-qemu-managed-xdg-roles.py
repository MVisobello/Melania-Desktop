#!/usr/bin/env python3
"""Verify actual QEMU framebuffer pixels at six *real* xdg role transitions.

This is a host-side QMP reader, not a mock visual preview. Each frame must
be captured during the corresponding Rust KMS FB_ID-acknowledged hold.
Only the VM guest emits its unique frame markers, and all pixel comparisons
are made against the exact wl_shm colors created by the Wayland client.
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
FRAMES = (3, 5, 7, 9, 13, 14)
EXIT_MARKER = "MELANIA_QEMU_NATIVE_GUEST_EXIT="
MARKER = "MELANIA_RUST_CONTINUOUS_MANAGED_ROLE_FRAME_VISIBLE={}"


def client_color(index: int) -> tuple[int, int, int]:
    return (
        30 + (index * 17) % 200,
        40 + (index * 31) % 190,
        25 + (index * 43) % 210,
    )


def expected_samples(frame: int) -> dict[str, tuple[tuple[int, int], tuple[int, int, int]]]:
    if frame not in FRAMES:
        raise ValueError("unrecognized managed-role QEMU frame")
    foreground = client_color(frame)
    second = client_color(frame - 1)
    background = (27, 33, 58)
    # A starts at (160,120) with 360x224; fullscreen and maximized
    # A fill 1280x800, while B remains in front at (650,190), 360x224.
    # B is intentionally not redrawn during the role transition of A.
    # Frames 13 and 14 have NO new Wayland image: they must reuse A11/B12
    # while the IPC shell hides and restores A's retained buffer.
    samples = {"B_retained_on_top": ((700, 250), client_color(12) if frame>=13 else second)}
    if frame in (13, 14):
        a_color = client_color(11) if frame==14 else background
        samples.update({
            "A_retained_or_hidden": ((200, 200), a_color),
            "A_rightmost_or_hidden": ((479, 200), a_color),
            "A_bottommost_or_hidden": ((200, 343), a_color),
            "A_outside_right": ((520, 200), background),
            "A_outside_bottom": ((200, 344), background),
            "B_retained_last_buffer": ((1000, 250), client_color(12)),
            "outside_both": ((40, 40), background),
        })
    elif frame in (3, 7):
        samples.update({
            "A_maximized_top_left": ((40, 40), foreground),
            "A_maximized_far_right": ((1250, 50), foreground),
            "A_maximized_bottom_right": ((1200, 700), foreground),
            "B_right_edge": ((1009, 250), second),
            "A_behind_B_edge": ((1010, 250), foreground),
        })
    else:
        samples.update({
            "background_outside_restored_A": ((40, 40), background),
            "A_restored_inside": ((200, 200), foreground),
            "A_restored_rightmost": ((519, 200), foreground),
            "A_outside_right": ((520, 200), background),
            "A_restored_bottommost": ((200, 343), foreground),
            "A_outside_bottom": ((200, 344), background),
            "B_left_edge": ((650, 250), second),
        })
    return samples


def verify_frame(data: bytes, frame: int) -> dict[str, tuple[int, int, int]]:
    header = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", data)
    if header is None or tuple(map(int, header.groups())) != (W, H):
        raise ValueError("QMP framebuffer is not 1280x800 PPM/P6")
    offset = header.end()
    if len(data) != offset + W * H * 3:
        raise ValueError("QMP framebuffer has truncated or extra bytes")
    observed: dict[str, tuple[int, int, int]] = {}
    for label, (point, expected) in expected_samples(frame).items():
        x, y = point
        address = offset + (y * W + x) * 3
        pixel = tuple(data[address:address + 3])
        observed[label] = pixel
        if any(abs(actual - wanted) > 18 for actual, wanted in zip(pixel, expected)):
            raise ValueError(
                f"frame {frame} {label} at {point}: got {pixel}, expected {expected}"
            )
    return observed


def capture(qmp: Path, target: Path) -> None:
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.settimeout(6)
        sock.connect(str(qmp))
        stream = sock.makefile("rwb", buffering=0)
        if "QMP" not in json.loads(stream.readline()):
            raise RuntimeError("QMP handshake failed")
        for command in (
            {"execute": "qmp_capabilities"},
            {"execute": "screendump", "arguments": {"filename": str(target), "format": "ppm"}},
        ):
            stream.write((json.dumps(command) + "\n").encode())
            for _ in range(50):
                response = json.loads(stream.readline())
                if "error" in response:
                    raise RuntimeError(f"QEMU screendump failed: {response['error']}")
                if "return" in response:
                    break
            else:
                raise RuntimeError("QMP command timed out")
    if not target.is_file():
        raise RuntimeError("QMP did not produce a real screenshot")


def wait_for_frame(serial: Path, frame: int, deadline: float) -> None:
    marker = MARKER.format(frame)
    while time.monotonic() < deadline:
        content = serial.read_text(errors="replace") if serial.exists() else ""
        if marker in content.splitlines():
            return
        if EXIT_MARKER in content:
            raise RuntimeError(f"VM ended before live KMS managed-role frame {frame}")
        time.sleep(0.04)
    raise TimeoutError(f"frame {frame} never reached virtual CRTC")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--serial", type=Path, required=True)
    parser.add_argument("--qmp", type=Path, required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=950)
    args = parser.parse_args()
    deadline = time.monotonic() + args.timeout
    args.out_dir.mkdir(parents=True, exist_ok=True)

    for frame in FRAMES:
        wait_for_frame(args.serial, frame, deadline)
        target = args.out_dir / f"managed-role-frame{frame}.ppm"
        error = "no QMP screenshot"
        for _ in range(8):
            try:
                capture(args.qmp, target)
                pixels = verify_frame(target.read_bytes(), frame)
                print(f"MELANIA_QEMU_MANAGED_XDG_FRAME{frame}_PIXELS_VERIFIED=true {pixels}",
                      flush=True)
                break
            except (OSError, ValueError, RuntimeError, EOFError) as exc:
                error = str(exc)
                time.sleep(0.1)
        else:
            raise RuntimeError(f"managed xdg frame {frame} pixels failed: {error}")

    print("MELANIA_QEMU_MANAGED_XDG_FOUR_FRAMES_VERIFIED=true", flush=True)
    print("MELANIA_QEMU_MANAGED_XDG_SIX_FRAMES_VERIFIED=true", flush=True)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError, TimeoutError, EOFError) as exc:
        print("MELANIA_QEMU_MANAGED_XDG_FOUR_FRAMES_VERIFIED=false: " + str(exc),
              file=sys.stderr)
        raise SystemExit(1)
