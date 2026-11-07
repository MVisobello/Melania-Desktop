#!/usr/bin/env python3
"""Stage130: real disposable-QEMU tablet hover over two Wayland clients.
No virtual click, no key, no IPC or fabricated focus event is permitted.
"""
from __future__ import annotations
import argparse
import json
import re
import socket
import sys
import time
from pathlib import Path

READY="MELANIA_QEMU_HOVER_TWO_WAYLAND_SURFACES_READY=true"
EXIT="MELANIA_QEMU_NATIVE_GUEST_EXIT="
W,H=1280,800

def after_ready(serial:Path,deadline:float)->str:
    while time.monotonic()<deadline:
        content=serial.read_text(errors="replace") if serial.exists() else ""
        if READY in content:
            return content[content.rfind(READY)+len(READY):]
        if EXIT in content:
            raise RuntimeError("guest ended before hover Wayland surfaces appeared")
        time.sleep(.08)
    raise TimeoutError("hover Wayland surfaces never became ready")

def until(serial:Path,marker:str,deadline:float)->None:
    while time.monotonic()<deadline:
        body=after_ready(serial,deadline)
        if marker in body:
            return
        if EXIT in body:
            raise RuntimeError("guest exited before "+marker)
        time.sleep(.08)
    raise TimeoutError("missing real libinput/Wayland receipt: "+marker)

def qmp(path:Path,request:dict)->None:
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as sock:
        sock.settimeout(8)
        sock.connect(str(path))
        io=sock.makefile("rwb",buffering=0)
        if "QMP" not in json.loads(io.readline()):
            raise RuntimeError("private QEMU monitor greeting absent")
        for command in ({"execute":"qmp_capabilities"},request):
            io.write((json.dumps(command)+"\n").encode())
            for _ in range(80):
                result=json.loads(io.readline())
                if "error" in result:
                    raise RuntimeError("QMP refused request: "+str(result["error"]))
                if "return" in result:
                    break
            else:
                raise RuntimeError("QMP did not acknowledge tablet/screendump")

def move(path:Path,x:int,y:int)->None:
    if not (0<=x<W and 0<=y<H):
        raise ValueError("tablet coordinate outside guest output")
    # Two ABS axes only: NO button event at any point in this acceptance test.
    events=[
        {"type":"abs","data":{"axis":"x","value":round(x*32767/(W-1))}},
        {"type":"abs","data":{"axis":"y","value":round(y*32767/(H-1))}},
    ]
    qmp(path,{"execute":"input-send-event","arguments":{"events":events}})
    time.sleep(.18)

def verify_real_crtc_ppm(path:Path)->None:
    raw=path.read_bytes()
    m=re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s",raw)
    if not m or tuple(map(int,m.groups()))!=(W,H):
        raise ValueError("genuine GPU screendump does not have 1280x800 pixels")
    off=m.end()
    if len(raw)!=off+W*H*3:
        raise ValueError("truncated QEMU CRTC screenshot")
    def px(x:int,y:int)->tuple[int,int,int]:
        idx=off+3*(y*W+x)
        return tuple(raw[idx:idx+3])
    for label,(x,y),expected in (
        ("background",(40,40),(27,33,58)),
        ("client_A",(200,200),(217,191,78)),
        ("client_B",(740,260),(34,222,121)),
    ):
        actual=px(x,y)
        if any(abs(a-b)>18 for a,b in zip(actual,expected)):
            raise ValueError(f"{label}: live GPU {actual}, expected client pixels {expected}")

def main()->int:
    p=argparse.ArgumentParser()
    p.add_argument("--serial",type=Path,required=True)
    p.add_argument("--qmp",type=Path,required=True)
    p.add_argument("--output",type=Path,required=True)
    p.add_argument("--timeout",type=float,default=1350)
    args=p.parse_args()
    deadline=time.monotonic()+args.timeout
    after_ready(args.serial,deadline)
    qmp(args.qmp,{"execute":"screendump","arguments":{
        "filename":str(args.output.resolve()),"format":"ppm"}})
    verify_real_crtc_ppm(args.output)
    print("MELANIA_QEMU_HOVER_CRTC12_REAL_CLIENT_PIXELS=true",flush=True)
    # B then A, exactly physical tablet movement with no mouse clicks.
    move(args.qmp,750,280)
    until(args.serial,"MELANIA_NATIVE_FOCUS_FOLLOWS_POINTER=2",deadline)
    until(args.serial,"MELANIA_QEMU_HOVER_REAL_WL_KEYBOARD_ENTER=B",deadline)
    move(args.qmp,250,230)
    until(args.serial,"MELANIA_NATIVE_FOCUS_FOLLOWS_POINTER=1",deadline)
    until(args.serial,"MELANIA_QEMU_HOVER_REAL_WL_KEYBOARD_ENTER=A",deadline)
    print("MELANIA_QEMU_HOVER_QMP_NO_CLICK_PASS=true",flush=True)
    return 0

if __name__=="__main__":
    try:
        raise SystemExit(main())
    except (OSError,TimeoutError,ValueError,RuntimeError,EOFError) as exc:
        print("MELANIA_QEMU_HOVER_QMP_NO_CLICK_PASS=false: "+str(exc),file=sys.stderr)
        raise SystemExit(1)
