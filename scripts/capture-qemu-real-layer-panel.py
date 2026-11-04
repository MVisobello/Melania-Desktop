#!/usr/bin/env python3
"""Capture genuine QMP pixels of a Wayland layer-shell panel over xdg."""
from __future__ import annotations
import argparse,json,re,socket,sys,time
from pathlib import Path
W,H=1280,800
MARKER="MELANIA_RUST_CONTINUOUS_LAYER_FRAME_VISIBLE=12"
EXIT="MELANIA_QEMU_NATIVE_GUEST_EXIT="

def color(index:int)->tuple[int,int,int]:
    return (30+(index*17)%200,40+(index*31)%190,25+(index*43)%210)

def verify(data:bytes)->dict[str,tuple[int,int,int]]:
    m=re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s",data)
    if not m or tuple(map(int,m.groups()))!=(W,H):
        raise ValueError("wrong QEMU framebuffer size")
    offset=m.end()
    if len(data)!=offset+W*H*3:raise ValueError("truncated QMP output")
    checks={
        "background_above_panel":((10,759),(27,33,58)),
        "panel_start_bottom":((10,760),color(12)),
        "panel_middle":((700,780),color(12)),
        "panel_last_pixel":((1279,799),color(12)),
        "xdg_retained_from_frame11":((200,200),color(11)),
        "xdg_bottom_edge":((200,343),color(11)),
        "xdg_outside":((200,344),(27,33,58)),
        "no_panel_leak_top":((1000,700),(27,33,58)),
    }
    found={}
    for name,((x,y),expected) in checks.items():
        start=offset+(y*W+x)*3
        got=tuple(data[start:start+3]);found[name]=got
        if any(abs(a-b)>18 for a,b in zip(got,expected)):
            raise ValueError(f"{name}: real pixel {got} != {expected}")
    return found

def capture(qmp:Path,path:Path)->None:
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as s:
        s.settimeout(6);s.connect(str(qmp))
        io=s.makefile("rwb",buffering=0)
        if "QMP" not in json.loads(io.readline()):raise RuntimeError("QMP greeting missing")
        for cmd in (
            {"execute":"qmp_capabilities"},
            {"execute":"screendump","arguments":{"filename":str(path),"format":"ppm"}},
        ):
            io.write((json.dumps(cmd)+"\n").encode())
            for _ in range(50):
                msg=json.loads(io.readline())
                if "error" in msg:raise RuntimeError(f"QMP command error: {msg['error']}")
                if "return" in msg:break
            else:raise RuntimeError("QMP response missing")
    if not path.is_file():raise RuntimeError("real screenshot absent")

def main()->int:
    p=argparse.ArgumentParser()
    p.add_argument("--serial",type=Path,required=True)
    p.add_argument("--qmp",type=Path,required=True)
    p.add_argument("--output",type=Path,required=True)
    p.add_argument("--timeout",type=float,default=950)
    a=p.parse_args()
    deadline=time.monotonic()+a.timeout
    while time.monotonic()<deadline:
        log=a.serial.read_text(errors="replace") if a.serial.exists() else ""
        if MARKER in log.splitlines():break
        if EXIT in log:raise RuntimeError("VM exited before layer KMS proof")
        time.sleep(.06)
    else:raise TimeoutError("real layer frame12 was never presented")
    last="no image"
    for _ in range(10):
        try:
            capture(a.qmp,a.output)
            actual=verify(a.output.read_bytes())
            print(f"MELANIA_QEMU_REAL_LAYER_PANEL_PIXELS=true {actual}",flush=True)
            return 0
        except (OSError,RuntimeError,ValueError,EOFError) as e:
            last=str(e);time.sleep(.12)
    raise RuntimeError("real layer/xdg QMP verification failed: "+last)

if __name__=="__main__":
    try:raise SystemExit(main())
    except (OSError,RuntimeError,ValueError,TimeoutError,EOFError) as e:
        print("MELANIA_QEMU_REAL_LAYER_PANEL_PIXELS=false: "+str(e),file=sys.stderr)
        raise SystemExit(1)
