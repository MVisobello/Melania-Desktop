#!/usr/bin/env python3
"""Stage41: screenshot ACTUAL QEMU GPU after a DIFFERENT xdg_toplevel reopens."""
from __future__ import annotations
import argparse,json,re,socket,sys,time
from pathlib import Path

W,H=1280,800
MARKER="MELANIA_RUST_REOPEN_FRAME6_ACTIVE=true"
EXIT="MELANIA_QEMU_NATIVE_GUEST_EXIT="

def screendump(qmp:Path,output:Path)->None:
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as sock:
        sock.settimeout(6)
        sock.connect(str(qmp))
        io=sock.makefile("rwb",buffering=0)
        if "QMP" not in json.loads(io.readline()):
            raise RuntimeError("QMP handshake not received")
        for command in (
            {"execute":"qmp_capabilities"},
            {"execute":"screendump","arguments":{"filename":str(output),"format":"ppm"}},
        ):
            io.write((json.dumps(command)+"\n").encode())
            for _ in range(50):
                result=json.loads(io.readline())
                if "error" in result:
                    raise RuntimeError("QMP rejected screenshot: "+str(result["error"]))
                if "return" in result: break
            else:
                raise RuntimeError("QMP screenshot reply not received")
    if not output.is_file():
        raise RuntimeError("QEMU has not written screenshot")

def verify(data:bytes)->dict[str,tuple[int,int,int]]:
    match=re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s",data)
    if not match or tuple(map(int,match.groups()))!=(W,H):
        raise ValueError("wrong real virtual GPU framebuffer resolution")
    offset=match.end()
    if len(data)!=offset+W*H*3:
        raise ValueError("QEMU screenshot truncated")
    def rgb(x:int,y:int)->tuple[int,int,int]:
        pos=offset+(y*W+x)*3
        return tuple(data[pos:pos+3])
    # A is now a NEW genuine wl_shm client at slot 1; B remains moved at
    # (740,270). Pixels are sampled from the live QEMU DRM framebuffer.
    expected={
        "background":((40,40),(27,33,58)),
        "window_A_new_border":((157,120),(65,171,216)),
        "window_A_reopened_left":((250,232),(190,80,160)),
        "window_A_reopened_right":((440,232),(245,175,55)),
        "window_B_old_cleared":((700,250),(27,33,58)),
        "window_B_new_border":((737,270),(65,171,216)),
        "window_B_moved_left":((820,380),(73,126,237)),
        "window_B_moved_right":((1000,380),(247,211,48)),
    }
    found={}
    for key,(point,want) in expected.items():
        got=rgb(*point)
        found[key]=got
        if any(abs(a-b)>18 for a,b in zip(got,want)):
            raise ValueError(f"{key} pixel {got} != {want}; no proven GPU redraw")
    return found

def main()->int:
    parser=argparse.ArgumentParser()
    parser.add_argument("--serial",type=Path,required=True)
    parser.add_argument("--qmp",type=Path,required=True)
    parser.add_argument("--output",type=Path,required=True)
    parser.add_argument("--timeout",type=float,default=950)
    args=parser.parse_args()
    end=time.monotonic()+args.timeout
    while time.monotonic()<end:
        log=args.serial.read_text(errors="replace") if args.serial.exists() else ""
        if MARKER in log: break
        if EXIT in log: raise RuntimeError("VM exited before sixth reopen KMS marker")
        time.sleep(.08)
    else: raise TimeoutError("sixth KMS framebuffer never became active")
    last_error=""
    for _ in range(7):
        try:
            screendump(args.qmp,args.output)
            found=verify(args.output.read_bytes())
            print(f"MELANIA_QEMU_REOPEN_FRAME6_PIXELS_VERIFIED=true {found}",flush=True)
            return 0
        except (OSError,ValueError,RuntimeError) as error:
            last_error=str(error)
            time.sleep(.2)
    raise RuntimeError("unable to verify real GPU reopen framebuffer: "+last_error)

if __name__=="__main__":
    try:
        raise SystemExit(main())
    except (OSError,ValueError,RuntimeError,TimeoutError,EOFError) as error:
        print("MELANIA_QEMU_REOPEN_FRAME6_PIXELS_VERIFIED=false: "+str(error),file=sys.stderr)
        raise SystemExit(1)
