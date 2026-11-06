#!/usr/bin/env python3
"""QMP tablet -> authenticated xdg_toplevel.move -> edge drop -> real DRM pixels.

This requires guest libinput, Wayland client buffer commits and QMP screen capture.
"""
from __future__ import annotations
import argparse, json, re, socket, sys, time
from pathlib import Path

W,H=1280,800
BG=(27,33,58)
EXIT="MELANIA_QEMU_NATIVE_GUEST_EXIT="
START="MELANIA_QEMU_EDGE_DROP_STAGE_STARTED=true"

def wait(serial:Path,marker:str,deadline:float)->None:
    while time.monotonic()<deadline:
        log=serial.read_text(errors="replace") if serial.exists() else ""
        if START in log and marker in log[log.rfind(START):]:
            return
        if EXIT in log:
            raise RuntimeError("VM ended before "+marker)
        time.sleep(.06)
    raise TimeoutError("no actual guest marker "+marker)

def qmp(path:Path,command:dict)->None:
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as sock:
        sock.settimeout(8)
        sock.connect(str(path))
        io=sock.makefile("rwb",buffering=0)
        if "QMP" not in json.loads(io.readline()):
            raise RuntimeError("QMP greeting absent")
        for request in ({"execute":"qmp_capabilities"},command):
            io.write((json.dumps(request)+"\n").encode())
            for _ in range(80):
                result=json.loads(io.readline())
                if "error" in result:
                    raise RuntimeError("QMP error: "+str(result["error"]))
                if "return" in result:
                    break
            else:
                raise RuntimeError("QMP command not acknowledged")

def key(path:Path,keyname:str,down:bool)->None:
    qmp(path,{"execute":"input-send-event","arguments":{"events":[{
        "type":"key","data":{
            "key":{"type":"qcode","data":keyname},
            "down":down,
        },
    }]}})
    time.sleep(.12)

def tablet(path:Path,x:int,y:int)->None:
    if not (0<=x<W and 0<=y<H):
        raise ValueError("tablet coordinate outside output")
    for axis,value,limit in (("x",x,W-1),("y",y,H-1)):
        qmp(path,{"execute":"input-send-event","arguments":{"events":[{
            "type":"abs","data":{"axis":axis,"value":round(value*32767/limit)},
        }]}})
    time.sleep(.18)

def button(path:Path,down:bool)->None:
    qmp(path,{"execute":"input-send-event","arguments":{"events":[{
        "type":"btn","data":{"button":"left","down":down},
    }]}})
    time.sleep(.18)


def pixels(path:Path,samples:list[tuple[str,int,int,tuple[int,int,int]]])->None:
    data=path.read_bytes()
    m=re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s",data)
    if not m or tuple(map(int,m.groups()))!=(W,H):
        raise ValueError("not a real 1280x800 PPM")
    offset=m.end()
    if len(data)!=offset+W*H*3:
        raise ValueError("truncated or padded QEMU CRTC buffer")
    for name,x,y,expected in samples:
        i=offset+3*(y*W+x)
        observed=tuple(data[i:i+3])
        if any(abs(a-b)>18 for a,b in zip(observed,expected)):
            raise ValueError(f"{name} at {(x,y)}: {observed} != {expected}")

def screenshot(qmp_path:Path,output:Path,samples)->None:
    qmp(qmp_path,{"execute":"screendump","arguments":{
        "filename":str(output.resolve()),"format":"ppm"
    }})
    pixels(output,samples)

def main()->int:
    ap=argparse.ArgumentParser()
    ap.add_argument("--serial",type=Path,required=True)
    ap.add_argument("--qmp",type=Path,required=True)
    ap.add_argument("--out-dir",type=Path,required=True)
    ap.add_argument("--timeout",type=float,default=1350)
    args=ap.parse_args()
    deadline=time.monotonic()+args.timeout
    args.out_dir.mkdir(parents=True,exist_ok=True)
    wait(args.serial,"MELANIA_NATIVE_EDGE_DROP_CLIENT_FRAME1_ACK=true",deadline)
    wait(args.serial,"MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=1",deadline)
    screenshot(args.qmp,args.out_dir/"edge-drop-original-frame1.ppm",[
        ("original-client",200,200,(180,44,62)),
        ("original-background",40,40,BG),
        ("original-right-background",700,400,BG),
    ])
    # Only a real libinput press on this actual Wayland surface can
    # authorize the xdg_toplevel.move serial. No IPC/forged Wayland event.
    tablet(args.qmp,200,160)
    button(args.qmp,True)
    wait(args.serial,"MELANIA_NATIVE_EDGE_DROP_CLIENT_MOVE_REQUESTED=true",deadline)
    wait(args.serial,"MELANIA_NATIVE_XDG_CLIENT_MOVE_GRANTED=1",deadline)
    tablet(args.qmp,5,400)
    button(args.qmp,False)
    wait(args.serial,"MELANIA_NATIVE_EDGE_DROP_REQUEST=1 Snap(LeftHalf)",deadline)
    wait(args.serial,"MELANIA_NATIVE_EDGE_DROP_CLIENT_XDG_HALF_ACK=true",deadline)
    wait(args.serial,"MELANIA_NATIVE_EDGE_DROP_CLIENT_FRAME2_ACK=true",deadline)
    screenshot(args.qmp,args.out_dir/"edge-drop-half-frame2.ppm",[
        ("left-top-client",20,20,(46,175,219)),
        ("left-edge-client",630,400,(46,175,219)),
        ("right-background",650,400,BG),
        ("far-right-background",1120,420,BG),
    ])
    print("MELANIA_QEMU_EDGE_DROP_REAL_CRTC_HALF_PIXELS=true",flush=True)
    key(args.qmp,"f8",True)
    key(args.qmp,"f8",False)
    wait(args.serial,"MELANIA_NATIVE_EDGE_DROP_CLIENT_HOST_RELEASE_ACK=true",deadline)
    return 0

if __name__=="__main__":
    try:
        raise SystemExit(main())
    except (OSError,ValueError,RuntimeError,TimeoutError,EOFError) as exc:
        print("MELANIA_QEMU_EDGE_DROP_CRTC_PIXELS=false: "+str(exc),file=sys.stderr)
        raise SystemExit(1)
