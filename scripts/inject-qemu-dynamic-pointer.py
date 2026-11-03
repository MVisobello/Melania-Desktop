#!/usr/bin/env python3
"""QEMU-only: move *resized* real Wayland B using a physical virtual USB tablet.

After frame12 CRTC ACK, no more client commits occur. The only legitimate
frame13 is damage caused by libinput's real left-button drag of window B.
"""
from __future__ import annotations
import argparse
import json
import socket
import sys
import time
from pathlib import Path

EXIT = "MELANIA_QEMU_NATIVE_GUEST_EXIT="


def wait_marker(serial: Path, marker: str, until: float) -> None:
    while time.monotonic() < until:
        log = serial.read_text(errors="replace") if serial.exists() else ""
        if marker in log:
            return
        if EXIT in log:
            raise RuntimeError("guest ended before " + marker)
        time.sleep(.06)
    raise TimeoutError("missing guest marker " + marker)


def tablet(qmp: Path, x: int, y: int, button: str | None = None) -> None:
    if not 0 <= x < 1280 or not 0 <= y < 800:
        raise ValueError("tablet coordinates outside guest output")
    events = ([{"type": "btn", "data": {"button": "left", "down": button == "press"}}]
              if button is not None else [
                  {"type": "abs", "data": {"axis": "x", "value": round(x * 32767 / 1279)}},
                  {"type": "abs", "data": {"axis": "y", "value": round(y * 32767 / 799)}},
              ])
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.settimeout(6)
        sock.connect(str(qmp))
        io = sock.makefile("rwb", buffering=0)
        if "QMP" not in json.loads(io.readline()):
            raise RuntimeError("QMP greeting absent")
        for command in (
            {"execute": "qmp_capabilities"},
            {"execute": "input-send-event", "arguments": {"events": events}},
        ):
            io.write((json.dumps(command) + "\n").encode())
            for _ in range(50):
                reply = json.loads(io.readline())
                if "error" in reply:
                    raise RuntimeError("QMP rejected real USB tablet: " + str(reply["error"]))
                if "return" in reply:
                    break
            else:
                raise RuntimeError("QMP reply not received")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--serial", type=Path, required=True)
    parser.add_argument("--qmp", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=950)
    args = parser.parse_args()
    until = time.monotonic() + args.timeout
    # The resized B is now 220x180 at (650,190), not the old 360x224.
    # Earlier stages in the SAME guest also ACK frame12; this unique
    # marker is emitted only AFTER the dynamic scene reached real KMS ACK12.
    wait_marker(args.serial, "MELANIA_RUST_CONTINUOUS_DYNAMIC_POINTER_READY_AFTER_KMS12=true", until)
    tablet(args.qmp, 760, 200)
    time.sleep(.12)
    tablet(args.qmp, 760, 200, "press")
    time.sleep(.14)
    tablet(args.qmp, 850, 280)
    time.sleep(.12)
    tablet(args.qmp, 850, 280, "release")
    wait_marker(args.serial,
                "MELANIA_RUST_CONTINUOUS_DYNAMIC_POINTER_FRAME_VISIBLE=13", until)
    print("MELANIA_QEMU_DYNAMIC_LIBINPUT_DRAG_FRAME13=true", flush=True)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, TimeoutError, RuntimeError, EOFError) as exc:
        print("MELANIA_QEMU_DYNAMIC_LIBINPUT_DRAG_FRAME13=false: " + str(exc),
              file=sys.stderr)
        raise SystemExit(1)
