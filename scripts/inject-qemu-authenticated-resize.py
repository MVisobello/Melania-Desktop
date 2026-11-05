#!/usr/bin/env python3
"""QMP: physical virtual tablet -> authenticated XDG resize -> real GPU screenshot."""
from __future__ import annotations
import argparse,json,re,socket,sys,time
from pathlib import Path

EXIT="MELANIA_QEMU_NATIVE_GUEST_EXIT="
BG=(27,33,58)
PIXEL=(46,175,219)
W,H=1280,800

def wait_marker(serial:Path,needle:str,deadline:float)->str:
    while time.monotonic()<deadline:
        log=serial.read_text(errors="replace") if serial.exists() else ""
        if needle in log:
            return log
        if EXIT in log:
            raise RuntimeError("guest exited before "+needle)
        time.sleep(.08)
    raise TimeoutError("missing live QEMU marker: "+needle)

def qmp(qmp_path:Path,command:dict)->None:
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as sock:
        sock.settimeout(8)
        sock.connect(str(qmp_path))
        io=sock.makefile("rwb",buffering=0)
        if "QMP" not in json.loads(io.readline()):
            raise RuntimeError("QMP greeting missing")
        for request in ({"execute":"qmp_capabilities"},command):
            io.write((json.dumps(request)+"\n").encode())
            for _ in range(60):
                result=json.loads(io.readline())
                if "error" in result:
                    raise RuntimeError("QEMU rejected "+str(result["error"]))
                if "return" in result:
                    break
            else:
                raise RuntimeError("QMP request not acknowledged")

def tablet(path:Path,x:int,y:int,button:str|None=None)->None:
    if not 0<=x<W or not 0<=y<H:
        raise ValueError("tablet coordinates outside guest")
    events=([{"type":"btn","data":{"button":"left","down":button=="press"}}]
            if button is not None else [
              {"type":"abs","data":{"axis":"x","value":round(x*32767/(W-1))}},
              {"type":"abs","data":{"axis":"y","value":round(y*32767/(H-1))}},
            ])
    qmp(path,{"execute":"input-send-event","arguments":{"events":events}})

def ppm(path:Path)->tuple[bytes,int]:
    data=path.read_bytes()
    match=re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s",data)
    if not match or tuple(map(int,match.groups()))!=(W,H):
        raise ValueError("not a real 1280x800 QEMU PPM")
    offset=match.end()
    if len(data)!=offset+W*H*3:
        raise ValueError("truncated QEMU framebuffer")
    return data,offset

def main()->int:
    p=argparse.ArgumentParser()
    p.add_argument("--serial",required=True,type=Path)
    p.add_argument("--qmp",required=True,type=Path)
    p.add_argument("--output",required=True,type=Path)
    p.add_argument("--timeout",type=float,default=1350)
    args=p.parse_args()
    deadline=time.monotonic()+args.timeout
    wait_marker(args.serial,"MELANIA_NATIVE_AUTH_RESIZE_FRAME1_ACK=true",deadline)
    tablet(args.qmp,200,160)
    time.sleep(.18)
    tablet(args.qmp,200,160,"press")
    wait_marker(args.serial,"MELANIA_NATIVE_XDG_CLIENT_RESIZE_GRANTED=1 edge=TopLeft",deadline)
    tablet(args.qmp,240,180)
    # QMP ABS X and Y may be delivered as separate libinput events.
    # The first XDG configure may only resize one axis: never pass on it.
    rect=None
    while time.monotonic()<deadline:
        log=args.serial.read_text(errors="replace") if args.serial.exists() else ""
        if EXIT in log:
            raise RuntimeError("guest exited before final 2D XDG configure")
        rows=re.findall(r"MELANIA_NATIVE_XDG_RESIZE_CONFIGURE=1 x=(\d+) y=(\d+) w=(\d+) h=(\d+)",log)
        for parts in reversed(rows):
            x,y,w,h=map(int,parts)
            if x+w==520 and y+h==344 and 0<x<520 and 0<y<344 and 96<=w<360 and 72<=h<224:
                rect=(x,y,w,h)
                break
        if rect is not None:
            break
        time.sleep(.06)
    if rect is None:
        raise RuntimeError("no valid 2-axis TopLeft configure with anchored opposite edges")
    x,y,w,h=rect
    wait_marker(args.serial,"MELANIA_NATIVE_AUTH_RESIZE_FRAME2_ACK=true",deadline)
    qmp(args.qmp,{"execute":"screendump","arguments":{
        "filename":str(args.output.resolve()),"format":"ppm"}})
    data,offset=ppm(args.output)
    def color(px:int,py:int)->tuple[int,int,int]:
        i=offset+3*(py*W+px)
        return tuple(data[i:i+3])
    for label,point,expected in (
        ("new-client-pixels",(x+10,y+10),PIXEL),
        ("old-top-left-cleared",(165,125),BG),
        ("right-edge-cleared",(520,170),BG),
        ("outside-output-background",(40,40),BG),
    ):
        actual=color(*point)
        if any(abs(a-b)>18 for a,b in zip(actual,expected)):
            raise ValueError(f"{label}: actual={actual}, expected={expected}")
    print(f"MELANIA_QEMU_AUTH_RESIZE_CRTC_PIXELS=true x={x} y={y} w={w} h={h}",flush=True)
    tablet(args.qmp,240,180,"release")
    wait_marker(args.serial,"MELANIA_NATIVE_AUTH_RESIZE_FINAL_CONFIGURE_ACK=true",deadline)
    print("MELANIA_QEMU_AUTH_RESIZE_REAL_POINTER_RELEASE=true",flush=True)
    return 0

if __name__=="__main__":
    try:
        raise SystemExit(main())
    except (OSError,RuntimeError,TimeoutError,ValueError,EOFError) as exc:
        print("MELANIA_QEMU_AUTH_RESIZE_CRTC_PIXELS=false: "+str(exc),file=sys.stderr)
        raise SystemExit(1)
