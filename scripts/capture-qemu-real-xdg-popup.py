#!/usr/bin/env python3
"""QMP proof of actual XDG popup pixels and cleared CRTC after destruction.

No mock screen: screenshots come only from the current isolated VirtIO GPU.
The guest uses a true xdg_popup separate wl_surface and ACKed wl_shm buffers.
"""
from __future__ import annotations
import argparse
import json
import re
import socket
import sys
import time
from pathlib import Path

W,H=1280,800
BG=(27,33,58)
PARENT=(185,50,75)
POPUP=(245,180,50)
START="MELANIA_QEMU_REAL_XDG_POPUP_STAGE118_STARTED=true"
EXIT="MELANIA_QEMU_NATIVE_GUEST_EXIT="

def stage_log(serial:Path,deadline:float)->str:
    while time.monotonic()<deadline:
        log=serial.read_text(errors="replace") if serial.exists() else ""
        if START in log:
            return log[log.index(START)+len(START):]
        if EXIT in log:
            raise RuntimeError("guest exited before real popup stage")
        time.sleep(.1)
    raise TimeoutError("actual XDG popup QEMU stage not started")

def wait_marker(serial:Path,needle:str,deadline:float)->None:
    while time.monotonic()<deadline:
        text=stage_log(serial,deadline)
        if needle in text:
            return
        if EXIT in text:
            raise RuntimeError("guest exited before required popup marker "+needle)
        time.sleep(.06)
    raise TimeoutError("missing real XDG popup/DRM proof: "+needle)

def qmp_capture(qmp:Path,output:Path)->None:
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as sock:
        sock.settimeout(7)
        sock.connect(str(qmp))
        io=sock.makefile("rwb",buffering=0)
        if "QMP" not in json.loads(io.readline()):
            raise RuntimeError("invalid QMP greeting")
        for command in (
            {"execute":"qmp_capabilities"},
            {"execute":"screendump","arguments":{
                "filename":str(output.resolve()),"format":"ppm",
            }},
        ):
            io.write((json.dumps(command)+"\n").encode())
            for _ in range(60):
                answer=json.loads(io.readline())
                if "error" in answer:
                    raise RuntimeError("QMP refused virtual CRTC screenshot: "+str(answer["error"]))
                if "return" in answer:
                    break
            else:
                raise RuntimeError("no response to actual QMP screendump")
    if not output.is_file():
        raise RuntimeError("VirtIO GPU did not produce framebuffer screenshot")

def qmp_key(path:Path,which:str,pressed:bool)->None:
    # Actual QEMU USB keyboard -> guest libinput -> Smithay, not direct IPC.
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as sock:
        sock.settimeout(7)
        sock.connect(str(path))
        io=sock.makefile("rwb",buffering=0)
        if "QMP" not in json.loads(io.readline()):
            raise RuntimeError("invalid QMP greeting for physical Escape")
        request={"execute":"input-send-event","arguments":{"events":[{
            "type":"key","data":{"key":{"type":"qcode","data":which},
                                 "down":pressed},
        }]}}
        for cmd in ({"execute":"qmp_capabilities"},request):
            io.write((json.dumps(cmd)+"\n").encode())
            for _ in range(60):
                response=json.loads(io.readline())
                if "error" in response:
                    raise RuntimeError("QMP refused real USB key: "+str(response["error"]))
                if "return" in response:
                    break
            else:
                raise RuntimeError("QMP USB key not acknowledged")


def qmp_mouse(path:Path,action:str,x:int=220,y:int=180)->None:
    # Genuine QEMU USB tablet: normalized 0..32767 coordinates and a
    # physical left-button event. This is NOT a compositor control command.
    if not (0<=x<W and 0<=y<H):
        raise ValueError("physical popup pointer outside 1280x800 output")
    if action=="move":
        events=[
            {"type":"abs","data":{"axis":"x","value":round(x*32767/(W-1))}},
            {"type":"abs","data":{"axis":"y","value":round(y*32767/(H-1))}},
        ]
    elif action in ("press","release"):
        events=[{"type":"btn","data":{"button":"left","down":action=="press"}}]
    else:
        raise ValueError("unsupported QEMU mouse action")
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as sock:
        sock.settimeout(7)
        sock.connect(str(path))
        io=sock.makefile("rwb",buffering=0)
        if "QMP" not in json.loads(io.readline()):
            raise RuntimeError("invalid QMP greeting for real popup pointer")
        for cmd in ({"execute":"qmp_capabilities"},
                    {"execute":"input-send-event","arguments":{"events":events}}):
            io.write((json.dumps(cmd)+"\n").encode())
            for _ in range(60):
                response=json.loads(io.readline())
                if "error" in response:
                    raise RuntimeError("QMP rejected physical pointer input: "+str(response["error"]))
                if "return" in response:
                    break
            else:
                raise RuntimeError("QMP physical pointer response missing")


def verify(path:Path,popup_visible:bool)->None:
    blob=path.read_bytes()
    header=re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s",blob)
    if not header or tuple(map(int,header.groups()))!=(W,H):
        raise ValueError("wrong GPU 1280x800 output dimensions")
    offset=header.end()
    if len(blob)!=offset+W*H*3:
        raise ValueError("virtual CRTC screenshot truncated")
    def rgb(x:int,y:int)->tuple[int,int,int]:
        n=offset+3*(W*y+x)
        return tuple(blob[n:n+3])
    expected=(
        ("background",(40,40),BG),
        ("parent-uncovered",(200,165),PARENT),
        ("parent-overlaid-popup",(460,250),POPUP if popup_visible else PARENT),
        ("popup-outside-parent",(570,270),POPUP if popup_visible else BG),
        ("after-popup-edge",(625,270),BG),
    )
    for label,point,want in expected:
        got=rgb(*point)
        if any(abs(a-b)>18 for a,b in zip(got,want)):
            raise ValueError(f"{label}: actual {got}, expected {want}")

def capture_until(qmp:Path,path:Path,visible:bool,deadline:float)->None:
    last="no screenshot"
    while time.monotonic()<deadline:
        try:
            qmp_capture(qmp,path)
            verify(path,visible)
            return
        except (RuntimeError,ValueError,FileNotFoundError,OSError,EOFError) as error:
            last=str(error)
            time.sleep(.14)
    raise RuntimeError("real popup QMP screenshot never reached expected pixels: "+last)

def main()->int:
    p=argparse.ArgumentParser()
    p.add_argument("--serial",required=True,type=Path)
    p.add_argument("--qmp",required=True,type=Path)
    p.add_argument("--popup",required=True,type=Path)
    p.add_argument("--cleared",required=True,type=Path)
    p.add_argument("--timeout",default=1350,type=float)
    args=p.parse_args()
    deadline=time.monotonic()+args.timeout
    wait_marker(args.serial,"MELANIA_NATIVE_REAL_POPUP_GRAB_PARENT_READY=true",deadline)
    qmp_mouse(args.qmp,"move",220,180)
    qmp_mouse(args.qmp,"press")
    # Keep the physical button depressed until the compositor authenticates
    # xdg_popup.grab against the exact wl_pointer press serial.
    wait_marker(args.serial,"MELANIA_NATIVE_XDG_POPUP_GRAB_GRANTED=true",deadline)
    qmp_mouse(args.qmp,"release")
    print("MELANIA_QEMU_XDG_POPUP_REAL_USB_GRAB=true",flush=True)
    wait_marker(args.serial,"MELANIA_NATIVE_REAL_POPUP_CONFIGURE_ACK=280,115 180x100",deadline)
    wait_marker(args.serial,"MELANIA_NATIVE_XDG_POPUP_CLIENT_BUFFER_ACCEPTED=180x100",deadline)
    wait_marker(args.serial,"MELANIA_NATIVE_REAL_POPUP_CLIENT_FRAME2_ACK=true",deadline)
    wait_marker(args.serial,"MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=2",deadline)
    capture_until(args.qmp,args.popup,True,min(deadline,time.monotonic()+5))
    print("MELANIA_QEMU_XDG_POPUP_REAL_CRTC_PIXELS=true",flush=True)
    qmp_key(args.qmp,"esc",True)
    wait_marker(args.serial,"MELANIA_NATIVE_XDG_POPUP_ESCAPE_DISMISSED=2",deadline)
    wait_marker(args.serial,"MELANIA_NATIVE_REAL_POPUP_CLIENT_POPUP_DONE_AFTER_ESCAPE=true",deadline)
    qmp_key(args.qmp,"esc",False)
    print("MELANIA_QEMU_XDG_POPUP_USB_ESC_POPUP_DONE=true",flush=True)
    wait_marker(args.serial,"MELANIA_NATIVE_REAL_POPUP_CLIENT_DESTROYED=true",deadline)
    wait_marker(args.serial,"MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=3",deadline)
    capture_until(args.qmp,args.cleared,False,min(deadline,time.monotonic()+5))
    print("MELANIA_QEMU_XDG_POPUP_DESTROY_CRTC_CLEARED=true",flush=True)
    return 0

if __name__=="__main__":
    try:
        raise SystemExit(main())
    except (OSError,ValueError,RuntimeError,TimeoutError,EOFError) as error:
        print("MELANIA_QEMU_XDG_POPUP_REAL_CRTC_PIXELS=false: "+str(error),
              file=sys.stderr)
        raise SystemExit(1)
