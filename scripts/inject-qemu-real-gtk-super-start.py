#!/usr/bin/env python3
"""Inject a genuine USB Super key in the disposable Stage87 QEMU guest.

The third screenshot is still independently captured/validated from the live
DRM/KMS CRTC. This script cannot substitute a synthetic GTK screenshot or
send commands straight to the panel's private Unix socket.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import socket
import sys
import time

SECOND = "MELANIA_RUST_REAL_GTK_TWO_FRAME_VISIBLE=2"
THIRD = "MELANIA_RUST_REAL_GTK_START_FRAME_VISIBLE=3"
EXIT = "MELANIA_QEMU_NATIVE_GUEST_EXIT="


def await_serial(serial: Path, marker: str, deadline: float) -> None:
    while time.monotonic() < deadline:
        lines = serial.read_text(errors="replace").splitlines() if serial.exists() else []
        if marker in lines:
            return
        if any(line.startswith(EXIT) for line in lines):
            raise RuntimeError("QEMU guest exited before " + marker)
        time.sleep(0.06)
    raise TimeoutError("QEMU did not reach " + marker)


def send_usb_super(qmp: Path) -> None:
    def exchange(stream, request: dict) -> None:
        stream.write((json.dumps(request) + "\n").encode("utf-8"))
        for _ in range(50):
            answer = json.loads(stream.readline())
            if "error" in answer:
                raise RuntimeError("QMP keyboard event rejected: " + str(answer["error"]))
            if "return" in answer:
                return
        raise RuntimeError("QMP response unavailable")

    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as conn:
        conn.settimeout(6)
        conn.connect(str(qmp))
        stream = conn.makefile("rwb", buffering=0)
        if "QMP" not in json.loads(stream.readline()):
            raise RuntimeError("invalid private QMP greeting")
        exchange(stream, {"execute": "qmp_capabilities"})
        for down in (True, False):
            # QEMU emits an actual USB keyboard report; the VM's libinput
            # device must deliver the evdev key to the Rust Wayland bridge.
            exchange(stream, {
                "execute": "input-send-event",
                "arguments": {"events": [{
                    "type": "key",
                    "data": {"key": {"type": "qcode", "data": "meta_l"}, "down": down},
                }]},
            })
            time.sleep(0.15)


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--serial", type=Path, required=True)
    p.add_argument("--qmp", type=Path, required=True)
    p.add_argument("--timeout", type=float, default=950)
    args = p.parse_args()
    deadline = time.monotonic() + args.timeout
    await_serial(args.serial, SECOND, deadline)
    # The Stage85 watcher must have time to capture the genuine second CRTC
    # before the physical shortcut starts GTK's third overlay repaint.
    time.sleep(0.55)
    send_usb_super(args.qmp)
    print("MELANIA_QEMU_GTK_USB_SUPER_DOWN_UP=true", flush=True)
    await_serial(args.serial, THIRD, deadline)
    print("MELANIA_QEMU_GTK_REAL_SUPER_INPUT_PASS=true", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, TimeoutError, EOFError) as error:
        print("MELANIA_QEMU_GTK_REAL_SUPER_INPUT_PASS=false: " + str(error),
              file=sys.stderr)
        raise SystemExit(1)
