#!/usr/bin/env python3
"""Stage37: inspect actual GPU output AFTER real libinput drag, not a mock.

A private QEMU QMP screendump must show window A shifted +80,+70 from
its original origin, with both client wl_shm color halves unchanged.
Window B remains retained and the vacated pixels become background.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import re
import socket
import sys
import time

W,H=1280,800
MARKER="MELANIA_RUST_MULTI_FRAME4_ACTIVE=true"
EXIT="MELANIA_QEMU_NATIVE_GUEST_EXIT="

def sample(data:bytes,x:int,y:int)->tuple[int,int,int]:
    m=re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s",data)
    if not m or tuple(map(int,m.groups()))!=(W,H):
        raise ValueError("not an original 1280x800 QEMU PPM")
    start=m.end()
    if len(data)!=start+W*H*3:
        raise ValueError("QEMU screenshot is incomplete")
    p=start+(y*W+x)*3
    return tuple(data[p:p+3])

def validate(data:bytes)->dict[str,tuple[int,int,int]]:
    expected={
        "vacatedA":(27,33,58),
        "movedBorderA":(65,171,216),
        "movedLeftA":(222,64,118),
        "movedRightA":(117,238,90),
        "retainedLeftB":(73,126,237),
        "retainedRightB":(247,211,48),
        "background":(27,33,58),
    }
    points={
        "vacatedA":(170,160),
        "movedBorderA":(237,205),
        "movedLeftA":(270,250),
        "movedRightA":(450,250),
        "retainedLeftB":(750,280),
        "retainedRightB":(920,280),
        "background":(40,40),
    }
    found={name:sample(data,*coords) for name,coords in points.items()}
    for label,want in expected.items():
        got=found[label]
        if any(abs(a-b)>18 for a,b in zip(got,want)):
            raise ValueError(f"real GPU layout repaint {label}: {got} != {want}")
    return found

def screendump(qmp:Path,path:Path)->None:
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as sock:
        sock.settimeout(6)
        sock.connect(str(qmp))
        io=sock.makefile("rwb",buffering=0)
        if "QMP" not in json.loads(io.readline()):
            raise RuntimeError("not a QEMU QMP socket")
        for payload in (
            {"execute":"qmp_capabilities"},
            {"execute":"screendump","arguments":{"filename":str(path),"format":"ppm"}},
        ):
            io.write((json.dumps(payload)+"\n").encode())
            for _ in range(40):
                response=json.loads(io.readline())
                if "error" in response:
                    raise RuntimeError(f"QEMU rejected screendump: {response['error']}")
                if "return" in response:
                    break
            else:
                raise RuntimeError("QMP screenshot response missing")

def main()->int:
    p=argparse.ArgumentParser()
    p.add_argument("--serial",type=Path,required=True)
    p.add_argument("--qmp",type=Path,required=True)
    p.add_argument("--output",type=Path,required=True)
    p.add_argument("--timeout",type=float,default=950)
    a=p.parse_args()
    end=time.monotonic()+a.timeout
    while time.monotonic()<end:
        log=a.serial.read_text(errors="replace") if a.serial.exists() else ""
        if MARKER in log:break
        if EXIT in log:raise RuntimeError("guest exited before GPU drag repaint")
        time.sleep(.08)
    else:raise TimeoutError("fourth real GPU frame not displayed")
    errors=[]
    for _ in range(10):
        try:
            screendump(a.qmp,a.output)
            colors=validate(a.output.read_bytes())
            print(f"MELANIA_QEMU_LAYOUT_FRAME4_PIXELS=true {colors}",flush=True)
            print("MELANIA_QEMU_DRAG_VISIBLE_GPU_PASS=true",flush=True)
            return 0
        except (OSError,RuntimeError,ValueError) as error:
            errors.append(str(error))
            time.sleep(.3)
    raise RuntimeError(f"drag not visible in GPU frame: {errors[-1]}")

if __name__=="__main__":
    try:
        raise SystemExit(main())
    except (OSError,ValueError,RuntimeError,TimeoutError,EOFError) as error:
        print(f"MELANIA_QEMU_DRAG_VISIBLE_GPU_PASS=false: {error}",file=sys.stderr)
        raise SystemExit(1)
