#!/usr/bin/env python3
"""Real USB Super+arrows -> XDG configure -> owned wl_shm -> QMP CRTC pixels.

No synthetic scene or mocked screenshot satisfies the acceptance check.
"""
from __future__ import annotations
import argparse, json, re, socket, sys, time
from pathlib import Path

W,H=1280,800
BG=(27,33,58)
EXIT="MELANIA_QEMU_NATIVE_GUEST_EXIT="
START="MELANIA_QEMU_SUPER_SNAP_STAGE_STARTED=true"

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

def chord(path:Path,arrow:str)->None:
    key(path,"meta_l",True)
    key(path,arrow,True)
    key(path,arrow,False)
    key(path,"meta_l",False)

def chord_shift(path:Path,arrow:str)->None:
    # QEMU left Shift is qcode 'shift': this is an actual virtual USB event.
    key(path,"meta_l",True)
    key(path,"shift",True)
    key(path,arrow,True)
    key(path,arrow,False)
    key(path,"shift",False)
    key(path,"meta_l",False)

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
    wait(args.serial,"MELANIA_NATIVE_SNAP_CLIENT_FRAME1_ACK=true",deadline)
    # Do not inject a shortcut into an unmapped/unfocused XDG surface:
    # the real guest must have delivered wl_keyboard focus first.
    wait(args.serial,"MELANIA_NATIVE_FIRST_COMMITTED_XDG_FOCUSED=1",deadline)
    wait(args.serial,"MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=1",deadline)
    screenshot(args.qmp,args.out_dir/"super-snap-original-frame1.ppm",[
        ("original-client",200,200,(180,44,62)),
        ("original-background",40,40,BG),
        ("original-right-background",700,400,BG),
    ])
    chord(args.qmp,"left")
    wait(args.serial,"MELANIA_NATIVE_SUPER_SNAP_CONFIGURED=Some(LeftHalf) 640x800 at 0,0",deadline)
    wait(args.serial,"MELANIA_NATIVE_SNAP_CLIENT_XDG_HALF_ACK=true",deadline)
    wait(args.serial,"MELANIA_NATIVE_SNAP_CLIENT_FRAME2_ACK=true",deadline)
    wait(args.serial,"MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=2",deadline)
    screenshot(args.qmp,args.out_dir/"super-snap-half-frame2.ppm",[
        ("half-client-top-left",10,10,(46,175,219)),
        ("half-client-right-edge",639,500,(46,175,219)),
        ("right-half-background",640,500,BG),
        ("right-output-background",1100,500,BG),
    ])
    print("MELANIA_QEMU_SUPER_SNAP_HALF_CRTC_PIXELS=true",flush=True)
    chord(args.qmp,"down")
    wait(args.serial,"MELANIA_NATIVE_SUPER_SNAP_CONFIGURED=None 360x224 at 160,120",deadline)
    wait(args.serial,"MELANIA_NATIVE_SNAP_CLIENT_XDG_RESTORE_ACK=true",deadline)
    wait(args.serial,"MELANIA_NATIVE_SNAP_CLIENT_FRAME3_ACK=true",deadline)
    wait(args.serial,"MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=3",deadline)
    screenshot(args.qmp,args.out_dir/"super-snap-restored-frame3.ppm",[
        ("restored-client",200,200,(34,201,94)),
        ("restored-original-left",160,120,(34,201,94)),
        ("outside-restore-upper-left",10,10,BG),
        ("outside-restore-after-right",520,200,BG),
        ("outside-restore-after-bottom",200,344,BG),
    ])
    print("MELANIA_QEMU_SUPER_SNAP_RESTORE_CRTC_PIXELS=true",flush=True)
    chord_shift(args.qmp,"right")
    wait(args.serial,"MELANIA_NATIVE_SUPER_SNAP_CONFIGURED=Some(RightThird) 427x800 at 853,0",deadline)
    wait(args.serial,"MELANIA_NATIVE_SNAP_CLIENT_XDG_RIGHT_THIRD_ACK=true",deadline)
    wait(args.serial,"MELANIA_NATIVE_SNAP_CLIENT_FRAME4_ACK=true",deadline)
    wait(args.serial,"MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=4",deadline)
    screenshot(args.qmp,args.out_dir/"super-snap-right-third-frame4.ppm",[
        ("right-third-left-background",852,500,BG),
        ("right-third-left-edge",853,500,(190,110,200)),
        ("right-third-right-edge",1279,500,(190,110,200)),
        ("right-third-top",900,10,(190,110,200)),
    ])
    print("MELANIA_QEMU_SUPER_SNAP_RIGHT_THIRD_CRTC_PIXELS=true",flush=True)
    chord_shift(args.qmp,"down")
    wait(args.serial,"MELANIA_NATIVE_SNAP_CLIENT_XDG_THIRD_RESTORE_ACK=true",deadline)
    wait(args.serial,"MELANIA_NATIVE_SNAP_CLIENT_FRAME5_ACK=true",deadline)
    wait(args.serial,"MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=5",deadline)
    screenshot(args.qmp,args.out_dir/"super-snap-third-restored-frame5.ppm",[
        ("third-restored-left-background",853,500,BG),
        ("third-restored-window",200,200,(70,240,170)),
        ("third-restored-top-left",160,120,(70,240,170)),
        ("third-restored-outside-window",520,200,BG),
    ])
    print("MELANIA_QEMU_SUPER_SNAP_THIRD_RESTORE_CRTC_PIXELS=true",flush=True)
    key(args.qmp,"f8",True)
    key(args.qmp,"f8",False)
    wait(args.serial,"MELANIA_NATIVE_SNAP_CLIENT_HOST_RELEASE_ACK=true",deadline)
    return 0

if __name__=="__main__":
    try:
        raise SystemExit(main())
    except (OSError,ValueError,RuntimeError,TimeoutError,EOFError) as exc:
        print("MELANIA_QEMU_SUPER_SNAP_CRTC_PIXELS=false: "+str(exc),file=sys.stderr)
        raise SystemExit(1)
