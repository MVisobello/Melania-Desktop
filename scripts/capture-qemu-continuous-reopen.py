#!/usr/bin/env python3
"""QMP proof: real continuous KMS frame 13 closes A and frame 14 reopens A."""
from __future__ import annotations
import argparse,json,re,socket,sys,time
from pathlib import Path
W,H=1280,800
EXIT="MELANIA_QEMU_NATIVE_GUEST_EXIT="

def screendump(qmp:Path,target:Path)->None:
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as sock:
        sock.settimeout(6)
        sock.connect(str(qmp))
        io=sock.makefile("rwb",buffering=0)
        if "QMP" not in json.loads(io.readline()):
            raise RuntimeError("QMP handshake missing")
        for request in (
            {"execute":"qmp_capabilities"},
            {"execute":"screendump","arguments":{"filename":str(target),"format":"ppm"}},
        ):
            io.write((json.dumps(request)+"\n").encode())
            for _ in range(50):
                result=json.loads(io.readline())
                if "error" in result:raise RuntimeError("QMP screendump failed: "+str(result["error"]))
                if "return" in result:break
            else:raise RuntimeError("QMP screendump unanswered")
    if not target.is_file():raise RuntimeError("QMP image missing")

def verify(blob:bytes,frame:int)->dict[str,tuple[int,int,int]]:
    header=re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s",blob)
    if not header or tuple(map(int,header.groups()))!=(W,H):
        raise ValueError("wrong virtual GPU resolution")
    offset=header.end()
    if len(blob)!=offset+W*H*3:
        raise ValueError("truncated QEMU PPM image")
    def rgb(x:int,y:int)->tuple[int,int,int]:
        at=offset+(y*W+x)*3
        return tuple(blob[at:at+3])
    b_color=(30+(12*17)%200,40+(12*31)%190,25+(12*43)%210)
    a_color=(27,33,58) if frame==13 else (190,80,160)
    a_border=(27,33,58) if frame==13 else (65,171,216)
    points={
        "background":((40,40),(27,33,58)),
        "A_region":((200,200),a_color),
        "A_border":((157,120),a_border),
        "B_retained":((700,250),b_color),
        "B_border":((647,190),(65,171,216)),
    }
    values={}
    for name,(pos,wanted) in points.items():
        observed=rgb(*pos)
        values[name]=observed
        if any(abs(a-b)>18 for a,b in zip(observed,wanted)):
            raise ValueError(f"frame {frame} {name}: saw {observed}, expected {wanted}")
    return values

def await_frame(serial:Path,index:int,deadline:float)->None:
    marker=f"MELANIA_RUST_CONTINUOUS_REOPEN_FRAME_VISIBLE={index}"
    while time.monotonic()<deadline:
        lines=serial.read_text(errors="replace").splitlines() if serial.exists() else []
        if marker in lines:return
        if any(line.startswith(EXIT) for line in lines):
            raise RuntimeError("VM exited before continuous close/reopen proof")
        time.sleep(.05)
    raise TimeoutError(f"continuous KMS frame {index} not visible")

def main()->int:
    p=argparse.ArgumentParser()
    p.add_argument("--serial",type=Path,required=True)
    p.add_argument("--qmp",type=Path,required=True)
    p.add_argument("--close",type=Path,required=True)
    p.add_argument("--reopen",type=Path,required=True)
    p.add_argument("--timeout",type=float,default=950)
    a=p.parse_args()
    deadline=time.monotonic()+a.timeout
    for index,path in ((13,a.close),(14,a.reopen)):
        await_frame(a.serial,index,deadline)
        last=""
        for _ in range(9):
            try:
                screendump(a.qmp,path)
                observed=verify(path.read_bytes(),index)
                print(f"MELANIA_QEMU_CONTINUOUS_FRAME{index}_PIXELS_VERIFIED=true {observed}",flush=True)
                break
            except (OSError,ValueError,RuntimeError,EOFError) as err:
                last=str(err)
                time.sleep(.12)
        else:raise RuntimeError(f"real GPU frame {index} failed pixel check: {last}")
    return 0

if __name__=="__main__":
    try:raise SystemExit(main())
    except (OSError,ValueError,RuntimeError,TimeoutError,EOFError) as err:
        print("MELANIA_QEMU_CONTINUOUS_CLOSE_REOPEN_PIXELS_VERIFIED=false: "+str(err),file=sys.stderr)
        raise SystemExit(1)
