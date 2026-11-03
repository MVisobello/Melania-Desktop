#!/usr/bin/env python3
"""Verify TWO independently owned Wayland windows on actual QEMU DRM scanout."""
from __future__ import annotations
import argparse
import json
import re
import socket
import sys
import time
from pathlib import Path

W, H = 1280, 800
MARKER = "MELANIA_RUST_CONTINUOUS_TWO_WINDOW_FRAME_VISIBLE=12"
EXIT = "MELANIA_QEMU_NATIVE_GUEST_EXIT="

def capture(qmp: Path, output: Path) -> None:
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.settimeout(6)
        sock.connect(str(qmp))
        stream = sock.makefile("rwb", buffering=0)
        if "QMP" not in json.loads(stream.readline()):
            raise RuntimeError("QMP handshake unavailable")
        for command in (
            {"execute": "qmp_capabilities"},
            {"execute": "screendump", "arguments": {"filename": str(output), "format": "ppm"}},
        ):
            stream.write((json.dumps(command)+"\n").encode())
            for _ in range(50):
                response = json.loads(stream.readline())
                if "error" in response:
                    raise RuntimeError("QEMU rejected screenshot: "+str(response["error"]))
                if "return" in response:
                    break
            else:
                raise RuntimeError("QMP command did not finish")
    if not output.is_file():
        raise RuntimeError("QEMU screenshot missing")

def color(index: int) -> tuple[int, int, int]:
    return (30+(index*17)%200, 40+(index*31)%190, 25+(index*43)%210)

def verify(data: bytes) -> dict[str, tuple[int,int,int]]:
    header = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", data)
    if not header or tuple(map(int,header.groups()))!=(W,H):
        raise ValueError("incorrect QEMU framebuffer dimensions")
    pos=header.end()
    if len(data)!=pos+W*H*3:
        raise ValueError("truncated QMP screenshot")
    def rgb(x:int,y:int)->tuple[int,int,int]:
        off=pos+(y*W+x)*3
        return tuple(data[off:off+3])
    expected={
        "background":((40,40),(27,33,58)),
        "window_A_retained_from_frame11":((200,200),color(11)),
        "window_B_frame12":((700,250),color(12)),
        "window_A_border":((157,120),(65,171,216)),
        "window_B_border":((647,190),(65,171,216)),
    }
    result={}
    for name,(point,want) in expected.items():
        actual=rgb(*point)
        result[name]=actual
        if any(abs(a-b)>18 for a,b in zip(actual,want)):
            raise ValueError(f"{name}: actual {actual}, expected {want}")
    return result

def main()->int:
    p=argparse.ArgumentParser()
    p.add_argument("--serial",type=Path,required=True)
    p.add_argument("--qmp",type=Path,required=True)
    p.add_argument("--output",type=Path,required=True)
    p.add_argument("--timeout",type=float,default=950)
    args=p.parse_args()
    end=time.monotonic()+args.timeout
    while time.monotonic()<end:
        log=args.serial.read_text(errors="replace") if args.serial.exists() else ""
        if MARKER in log.splitlines():
            break
        if EXIT in log:
            raise RuntimeError("VM shut down before two-window KMS frame")
        time.sleep(.055)
    else:
        raise TimeoutError("two-window continuous frame never active")
    error="no image"
    for _ in range(9):
        try:
            capture(args.qmp,args.output)
            pixels=verify(args.output.read_bytes())
            print(f"MELANIA_QEMU_CONTINUOUS_TWO_WINDOWS_PIXELS_VERIFIED=true {pixels}",flush=True)
            return 0
        except (OSError,ValueError,RuntimeError,EOFError) as exc:
            error=str(exc)
            time.sleep(.12)
    raise RuntimeError("two-window actual GPU framebuffer verification failed: "+error)

if __name__=="__main__":
    try:
        raise SystemExit(main())
    except (OSError,ValueError,RuntimeError,TimeoutError,EOFError) as exc:
        print("MELANIA_QEMU_CONTINUOUS_TWO_WINDOWS_PIXELS_VERIFIED=false: "+str(exc),file=sys.stderr)
        raise SystemExit(1)
