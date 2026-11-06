#!/usr/bin/env python3
"""A real QMP USB keyboard -> libinput -> Smithay wl_keyboard focus proof.

The native client checks wl_keyboard.Enter for B and then A. The compositor
must additionally submit an authentic GPU/GBM/DRM frame 13 when A is raised,
and QMP captures retained A/B wl_shm pixels from the running virtual CRTC.
"""
from __future__ import annotations
import argparse
import json
import re
import socket
import sys
import time
from pathlib import Path

READY="MELANIA_QEMU_ALT_TAB_TWO_WAYLAND_SURFACES_READY=true"
EXIT="MELANIA_QEMU_NATIVE_GUEST_EXIT="
W,H=1280,800

def serial_after_ready(serial:Path,deadline:float)->str:
    while time.monotonic()<deadline:
        content=serial.read_text(errors="replace") if serial.exists() else ""
        if READY in content:
            return content[content.rfind(READY)+len(READY):]
        if EXIT in content:
            raise RuntimeError("guest exited before two genuine Wayland windows")
        time.sleep(.08)
    raise TimeoutError("two real client surfaces not ready for USB Alt+Tab")

def wait_after_ready(serial:Path,marker:str,deadline:float)->None:
    while time.monotonic()<deadline:
        body=serial_after_ready(serial,deadline)
        if marker in body:
            return
        if EXIT in body:
            raise RuntimeError("guest exited before "+marker)
        time.sleep(.08)
    raise TimeoutError("missing real USB/Wayland/DRM marker "+marker)

def qmp(path:Path,request:dict)->None:
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as sock:
        sock.settimeout(8)
        sock.connect(str(path))
        io=sock.makefile("rwb",buffering=0)
        if "QMP" not in json.loads(io.readline()):
            raise RuntimeError("private QMP greeting invalid")
        for command in ({"execute":"qmp_capabilities"},request):
            io.write((json.dumps(command)+"\n").encode())
            for _ in range(80):
                result=json.loads(io.readline())
                if "error" in result:
                    raise RuntimeError("QMP refused USB key or real screendump: "+str(result["error"]))
                if "return" in result:
                    break
            else:
                raise RuntimeError("QMP command never acknowledged")

def key(path:Path,which:str,pressed:bool)->None:
    qmp(path,{"execute":"input-send-event","arguments":{"events":[{
        "type":"key","data":{
            "key":{"type":"qcode","data":which},"down":pressed,
        },
    }]}})
    time.sleep(.12)

def verify_ppm(image:Path)->None:
    data=image.read_bytes()
    header=re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s",data)
    if not header or tuple(map(int,header.groups()))!=(W,H):
        raise ValueError("wrong dimensions in genuine VirtIO GPU screendump")
    offset=header.end()
    if len(data)!=offset+W*H*3:
        raise ValueError("incomplete real Virtual GPU screenshot")
    def pixel(x:int,y:int)->tuple[int,int,int]:
        i=offset+3*(W*y+x)
        return tuple(data[i:i+3])
    for label,point,expected in (
        ("background",(40,40),(27,33,58)),
        ("retained_client_A_frame11",(200,200),(217,191,78)),
        ("retained_client_B_frame12",(740,260),(34,222,121)),
        ("outside_client_B",(1060,490),(27,33,58)),
    ):
        actual=pixel(*point)
        if any(abs(a-b)>18 for a,b in zip(actual,expected)):
            raise ValueError(f"{label}: actual {actual} != client framebuffer {expected}")

def main()->int:
    p=argparse.ArgumentParser()
    p.add_argument("--serial",type=Path,required=True)
    p.add_argument("--qmp",type=Path,required=True)
    p.add_argument("--output",type=Path,required=True)
    p.add_argument("--timeout",type=float,default=1350)
    args=p.parse_args()
    deadline=time.monotonic()+args.timeout
    serial_after_ready(args.serial,deadline)
    # Only the Linux guest USB keyboard, never direct wl_keyboard or IPC.
    key(args.qmp,"alt",True)
    key(args.qmp,"tab",True)
    wait_after_ready(args.serial,"MELANIA_QEMU_ALT_TAB_REAL_WL_KEYBOARD_ENTER=B",deadline)
    wait_after_ready(args.serial,"MELANIA_NATIVE_KEYBOARD_ALT_TAB_FOCUS=2",deadline)
    key(args.qmp,"tab",False)
    key(args.qmp,"tab",True)
    wait_after_ready(args.serial,"MELANIA_QEMU_ALT_TAB_REAL_WL_KEYBOARD_ENTER=A",deadline)
    wait_after_ready(args.serial,"MELANIA_NATIVE_KEYBOARD_ALT_TAB_FOCUS=1",deadline)
    key(args.qmp,"tab",False)
    key(args.qmp,"alt",False)
    wait_after_ready(args.serial,"MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=13",deadline)
    qmp(args.qmp,{"execute":"screendump","arguments":{
        "filename":str(args.output.resolve()),"format":"ppm"}})
    verify_ppm(args.output)
    # Keep the physical frame13 screenshot proof before requesting close.
    key(args.qmp,"alt",True)
    key(args.qmp,"f4",True)
    wait_after_ready(args.serial,"MELANIA_NATIVE_KEYBOARD_ALT_F4_CLOSE_REQUESTED=1",deadline)
    wait_after_ready(args.serial,"MELANIA_QEMU_ALT_F4_REAL_XDG_CLOSE_RECEIVED=A",deadline)
    key(args.qmp,"f4",False)
    key(args.qmp,"alt",False)
    print("MELANIA_QEMU_ALT_F4_REAL_WAYLAND_CLOSE=true",flush=True)
    print("MELANIA_QEMU_ALT_TAB_REAL_WL_KEYBOARD_ENTER_A_B=true",flush=True)
    print("MELANIA_QEMU_ALT_TAB_REAL_KERNEL_CRTC_FRAME13_PIXELS=true",flush=True)
    return 0

if __name__=="__main__":
    try:
        raise SystemExit(main())
    except (OSError,RuntimeError,TimeoutError,ValueError,EOFError) as error:
        print("MELANIA_QEMU_ALT_TAB_REAL_KERNEL_CRTC_FRAME13_PIXELS=false: "+str(error),
              file=sys.stderr)
        raise SystemExit(1)
