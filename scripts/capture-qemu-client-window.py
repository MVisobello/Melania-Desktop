#!/usr/bin/env python3
"""Capture QMP pixels composited from an actual wl_shm Wayland client by Rust.

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

ACTIVE = "MELANIA_QEMU_CLIENT_WINDOW_SCANOUT_ACTIVE=true"
GUEST_EXIT = "MELANIA_QEMU_NATIVE_GUEST_EXIT="

def parse_ppm(data: bytes) -> tuple[int, int, dict[str,tuple[int,int,int]]]:
    header=re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s",data)
    if not header:
        raise ValueError("QMP did not produce P6 PPM")
    w,h=(int(v) for v in header.groups())
    if (w,h)!=(1280,800) or len(data)!=header.end()+w*h*3:
        raise ValueError(f"unexpected QEMU display dimensions/pixel count {w}x{h}")
    def sample(x:int,y:int)->tuple[int,int,int]:
        i=header.end()+(y*w+x)*3
        return tuple(data[i:i+3])
    return w,h,{
        "background":sample(40,40),
        "border":sample(157,120),
        "client_left":sample(250,232),
        "client_right":sample(430,232),
    }

def validate_scanout(data: bytes)->str:
    w,h,samples=parse_ppm(data)
    expected={
        "background":(27,33,58),
        "border":(65,171,216),
        "client_left":(245,108,34),
        "client_right":(16,205,155),
    }
    for key,value in expected.items():
        if any(abs(a-b)>18 for a,b in zip(samples[key],value)):
            raise ValueError(f"real Wayland client {key} pixel mismatch: {samples[key]} != {value}")
    return (f"MELANIA_QEMU_CLIENT_PIXELS_VERIFIED=true {w}x{h} "
            f"real_client_left={samples['client_left']} "
            f"real_client_right={samples['client_right']} "
            f"background={samples['background']} border={samples['border']}")

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
            raise RuntimeError(f"cannot capture real client scanout: {last_error}")
        if GUEST_EXIT in text:
            raise RuntimeError("guest exited without confirming a visible frame")
        time.sleep(0.25)
    raise TimeoutError("QEMU client-window scanout marker not observed before deadline")

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
        print(f"MELANIA_QEMU_CLIENT_PIXELS_VERIFIED=false: {error}",file=sys.stderr,flush=True)
        return 1
    print(result,flush=True)
    return 0

if __name__=="__main__":
    raise SystemExit(main())
