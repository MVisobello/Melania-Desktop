#!/usr/bin/env python3
"""Actual QMP screenshot of a real resized wl_shm client: fifth KMS frame."""
from pathlib import Path
import argparse,json,re,socket,sys,time

W,H=1280,800
def pixel(data:bytes,x:int,y:int)->tuple[int,int,int]:
    match=re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s",data)
    if not match or tuple(map(int,match.groups()))!=(W,H):
        raise ValueError("not a 1280x800 QMP GPU screenshot")
    start=match.end()
    if len(data)!=start+W*H*3:
        raise ValueError("short or synthetic screenshot payload")
    i=start+(y*W+x)*3
    return tuple(data[i:i+3])

def validate(data:bytes)->dict[str,tuple[int,int,int]]:
    coords={
        "new_right_expansion":(610,250),
        "new_bottom_expansion":(300,425),
        "retained_left_window":(270,250),
        "other_client":(750,280),
        "vacated_original":(170,160),
    }
    expect={
        "new_right_expansion":(117,238,90),
        "new_bottom_expansion":(222,64,118),
        "retained_left_window":(222,64,118),
        "other_client":(73,126,237),
        "vacated_original":(27,33,58),
    }
    found={name:pixel(data,*point) for name,point in coords.items()}
    for name,want in expect.items():
        if any(abs(x-y)>18 for x,y in zip(found[name],want)):
            raise ValueError(f"resized client GPU pixel {name}: {found[name]} != {want}")
    return found

def screendump(qmp:Path,out:Path)->None:
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as sock:
        sock.settimeout(6)
        sock.connect(str(qmp))
        io=sock.makefile("rwb",buffering=0)
        if "QMP" not in json.loads(io.readline()):
            raise RuntimeError("unexpected QMP greeting")
        for cmd in (
            {"execute":"qmp_capabilities"},
            {"execute":"screendump","arguments":{"filename":str(out),"format":"ppm"}},
        ):
            io.write((json.dumps(cmd)+"\n").encode())
            for _ in range(40):
                answer=json.loads(io.readline())
                if "error" in answer:raise RuntimeError(f"QMP error: {answer['error']}")
                if "return" in answer:break
            else:raise RuntimeError("QMP did not confirm screenshot")

def main()->int:
    p=argparse.ArgumentParser()
    p.add_argument("--serial",required=True,type=Path)
    p.add_argument("--qmp",required=True,type=Path)
    p.add_argument("--output",required=True,type=Path)
    p.add_argument("--timeout",type=float,default=950)
    a=p.parse_args()
    deadline=time.monotonic()+a.timeout
    while time.monotonic()<deadline:
        contents=a.serial.read_text(errors="replace") if a.serial.exists() else ""
        if "MELANIA_RUST_MULTI_FRAME5_ACTIVE=true" in contents:break
        if "MELANIA_QEMU_NATIVE_GUEST_EXIT=" in contents:
            raise RuntimeError("VM exited without real fifth KMS frame")
        time.sleep(.08)
    else:raise TimeoutError("resized buffer never reached virtual GPU")
    error=""
    for _ in range(10):
        try:
            screendump(a.qmp,a.output)
            result=validate(a.output.read_bytes())
            print(f"MELANIA_QEMU_RESIZED_FRAME5_PIXELS=true {result}",flush=True)
            print("MELANIA_QEMU_REAL_RESIZE_VISIBLE_GPU_PASS=true",flush=True)
            return 0
        except (OSError,ValueError,RuntimeError) as exc:
            error=str(exc)
            time.sleep(.3)
    raise RuntimeError(f"QMP resized frame not verified: {error}")

if __name__=="__main__":
    try:raise SystemExit(main())
    except (OSError,ValueError,RuntimeError,TimeoutError,EOFError) as error:
        print(f"MELANIA_QEMU_REAL_RESIZE_VISIBLE_GPU_PASS=false: {error}",file=sys.stderr)
        raise SystemExit(1)
