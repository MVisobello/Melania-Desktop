#!/usr/bin/env python3
"""QMP proof of two successive real Wayland wl_shm client frames on Rust KMS.

Only a private QEMU Unix monitor is accessed. The client owns the window
pixels; Rust Smithay copies them, Rust drm-rs presents two separate GEM FBs.
No host GPU/input device and no synthetic screenshot substitution.
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
FIRST="MELANIA_RUST_KMS_FRAME1_ACTIVE=true"
SECOND="MELANIA_RUST_KMS_FRAME2_ACTIVE=true"
GUEST_EXIT="MELANIA_QEMU_NATIVE_GUEST_EXIT="

def sample_ppm(data:bytes)->dict[str,tuple[int,int,int]]:
    header=re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s",data)
    if not header or tuple(int(x) for x in header.groups())!=(W,H):
        raise ValueError("expected 1280x800 binary QEMU PPM")
    start=header.end()
    if len(data)!=start+W*H*3:
        raise ValueError("incomplete RGB frame")
    def pixel(x:int,y:int)->tuple[int,int,int]:
        pos=start+(y*W+x)*3
        return tuple(data[pos:pos+3])
    return {
        "background":pixel(40,40),
        "border":pixel(157,120),
        "left":pixel(250,232),
        "right":pixel(430,232),
    }

def validate(data:bytes,frame:int)->dict[str,tuple[int,int,int]]:
    colors=sample_ppm(data)
    client={
        1:{"left":(245,108,34),"right":(16,205,155)},
        2:{"left":(73,126,237),"right":(247,211,48)},
    }
    expected={"background":(27,33,58),"border":(65,171,216),**client[frame]}
    for location,want in expected.items():
        got=colors[location]
        if any(abs(x-y)>18 for x,y in zip(got,want)):
            raise ValueError(f"Rust KMS frame {frame} {location} not from client: {got} != {want}")
    return colors

def screendump(qmp:Path,path:Path)->None:
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as sock:
        sock.settimeout(6)
        sock.connect(str(qmp))
        stream=sock.makefile("rwb",buffering=0)
        greeting=json.loads(stream.readline())
        if "QMP" not in greeting:
            raise RuntimeError("QMP greeting missing")
        def command(payload:dict)->None:
            stream.write((json.dumps(payload)+"\n").encode())
            for _ in range(30):
                reply=json.loads(stream.readline())
                if "error" in reply:
                    raise RuntimeError(f"QMP command rejected: {reply['error']}")
                if "return" in reply:
                    return
            raise RuntimeError("QMP reply missing")
        command({"execute":"qmp_capabilities"})
        command({"execute":"screendump",
                 "arguments":{"filename":str(path),"format":"ppm"}})
    if not path.is_file():
        raise RuntimeError("QMP screenshot not created")

def wait_marker(serial:Path,marker:str,deadline:float)->None:
    while time.monotonic()<deadline:
        content=serial.read_text(errors="replace") if serial.exists() else ""
        if marker in content:
            return
        if GUEST_EXIT in content:
            raise RuntimeError(f"guest finished before {marker}")
        time.sleep(.2)
    raise TimeoutError(f"native Rust KMS marker not observed: {marker}")

def watch(serial:Path,qmp:Path,first:Path,second:Path,timeout:float)->None:
    end=time.monotonic()+timeout
    for index,(marker,path) in enumerate(((FIRST,first),(SECOND,second)),start=1):
        wait_marker(serial,marker,end)
        # Markers are emitted only after Rust drm-rs verifies the current FB_ID.
        # A seven-second bounded hold allows a conservative retry on QMP.
        last_error=""
        for _ in range(9):
            try:
                screendump(qmp,path)
                pixels=validate(path.read_bytes(),index)
                print(f"MELANIA_QEMU_RUST_KMS_FRAME{index}_PIXELS=true {pixels}",flush=True)
                break
            except (OSError,RuntimeError,ValueError) as error:
                last_error=str(error)
                time.sleep(.3)
        else:
            raise RuntimeError(f"QMP frame{index} never displayed client pixels: {last_error}")
    if first.read_bytes()==second.read_bytes():
        raise RuntimeError("two KMS images were identical despite different client commits")
    print("MELANIA_QEMU_RUST_KMS_TWO_CAPTURE_PASS=true",flush=True)

def main()->int:
    p=argparse.ArgumentParser()
    p.add_argument("--serial",type=Path,required=True)
    p.add_argument("--qmp",type=Path,required=True)
    p.add_argument("--first",type=Path,required=True)
    p.add_argument("--second",type=Path,required=True)
    p.add_argument("--timeout",type=float,default=950)
    args=p.parse_args()
    try:
        watch(args.serial,args.qmp,args.first,args.second,args.timeout)
        return 0
    except (OSError,RuntimeError,ValueError,TimeoutError) as error:
        print(f"MELANIA_QEMU_RUST_KMS_TWO_CAPTURE_PASS=false: {error}",file=sys.stderr)
        return 1

if __name__=="__main__":
    raise SystemExit(main())
