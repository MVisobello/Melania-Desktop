#!/usr/bin/env python3
"""Capture actual QEMU virtual display pixels while the guest's KMS FB is live.

Pure-stdlib QMP client. No host DRM, KVM, guest disk or privileged APIs.
Only the sacrificial VM's QMP unix socket is contacted.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import socket
import sys
import time

ACTIVE = "MELANIA_QEMU_VISIBLE_SCANOUT_ACTIVE=true"
GUEST_EXIT = "MELANIA_QEMU_NATIVE_GUEST_EXIT="

def parse_ppm(data: bytes) -> tuple[int, int, tuple[int,int,int], tuple[int,int,int]]:
    # QEMU screendump outputs binary P6 PPM by default.
    header = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", data)
    if header is None:
        raise ValueError("QMP did not produce an ordinary 8-bit binary PPM")
    width, height = (int(v) for v in header.groups())
    if width < 640 or height < 480 or width > 1920 or height > 1080:
        raise ValueError(f"unexpected virtual display dimensions {width}x{height}")
    start = header.end()
    if len(data) != start + width * height * 3:
        raise ValueError("PPM pixel byte count incomplete")
    def sample(x: int, y: int) -> tuple[int,int,int]:
        offset = start + (y*width+x)*3
        return tuple(data[offset:offset+3])  # type: ignore[return-value]
    return width, height, sample(width//4, height//2), sample(3*width//4, height//2)

def validate_scanout(data: bytes) -> str:
    width,height,left,right = parse_ppm(data)
    # XRGB8888 pattern was painted by the QEMU guest (not by the host).
    if not (left[0] > 170 and left[1] < 100 and left[2] > 110
            and right[0] < 100 and right[1] > 160 and right[2] > 150):
        raise ValueError(f"visible guest pixels do not match expected pattern: {left=} {right=}")
    return f"MELANIA_QEMU_VISIBLE_PIXELS_VERIFIED=true {width}x{height} left={left} right={right}"

def qmp_capture(path: Path, output: Path) -> str:
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.settimeout(5)
        sock.connect(str(path))
        stream = sock.makefile("rwb", buffering=0)
        def read_reply() -> dict:
            for _ in range(30):
                line = stream.readline()
                if not line:
                    raise RuntimeError("QMP disconnected")
                data = json.loads(line)
                if "error" in data:
                    raise RuntimeError(f"QMP rejected command: {data['error']}")
                if "return" in data:
                    return data
            raise RuntimeError("QMP response not received")
        # Greeting first, then capability negotiation.
        greeting = json.loads(stream.readline())
        if "QMP" not in greeting:
            raise RuntimeError("QMP greeting missing")
        stream.write(b'{"execute":"qmp_capabilities"}\n')
        read_reply()
        command = {"execute":"screendump",
                   "arguments":{"filename":str(output),"format":"ppm"}}
        stream.write((json.dumps(command)+"\n").encode())
        read_reply()
    return validate_scanout(output.read_bytes())

def watch(serial: Path, qmp: Path, output: Path, deadline: float) -> str:
    end = time.monotonic()+deadline
    while time.monotonic() < end:
        text = serial.read_text(errors="replace") if serial.exists() else ""
        if ACTIVE in text:
            # The active marker is emitted only after the CRTC FB_ID changes.
            for _ in range(12):
                if qmp.exists():
                    try:
                        return qmp_capture(qmp, output)
                    except (OSError, ValueError, RuntimeError) as error:
                        last_error = str(error)
                else:
                    last_error = "QMP socket missing"
                time.sleep(0.3)
            raise RuntimeError(f"cannot capture guest's active scanout: {last_error}")
        if GUEST_EXIT in text:
            raise RuntimeError("guest exited without confirming a visible frame")
        time.sleep(0.25)
    raise TimeoutError("QEMU visible-frame marker not observed before deadline")

def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--serial", required=True, type=Path)
    parser.add_argument("--qmp", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--timeout", type=float, default=950)
    args = parser.parse_args()
    try:
        result=watch(args.serial,args.qmp,args.output,args.timeout)
    except (OSError, ValueError, RuntimeError, TimeoutError) as error:
        print(f"MELANIA_QEMU_VISIBLE_PIXELS_VERIFIED=false: {error}",file=sys.stderr,flush=True)
        return 1
    print(result,flush=True)
    return 0

if __name__=="__main__":
    raise SystemExit(main())
