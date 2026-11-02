#!/usr/bin/env python3
"""Stage38: move real Wayland window B using only private QEMU QMP tablet input.

The guest's Smithay/libinput bridge must generate a new KMS frame from
retained scene damage, not from a fourth client wl_shm buffer commit.
"""
from __future__ import annotations
import argparse,json
from pathlib import Path
import socket,sys,time

EXIT="MELANIA_QEMU_NATIVE_GUEST_EXIT="

def wait_marker(serial:Path, marker:str, deadline:float)->None:
    while time.monotonic()<deadline:
        log=serial.read_text(errors="replace") if serial.exists() else ""
        if marker in log:
            return
        if EXIT in log:
            raise RuntimeError("VM exited before "+marker)
        time.sleep(.08)
    raise TimeoutError("missing real guest marker: "+marker)

def pointer(qmp:Path,x:int,y:int,button:str|None=None)->None:
    if not (0<=x<1280 and 0<=y<800):
        raise ValueError("invalid QEMU screen coordinates")
    events=[
        {"type":"abs","data":{"axis":"x","value":round(x*32767/1279)}},
        {"type":"abs","data":{"axis":"y","value":round(y*32767/799)}},
    ]
    if button:
        events=[{"type":"btn","data":{"button":"left","down":button=="press"}}]
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as sock:
        sock.settimeout(6)
        sock.connect(str(qmp))
        io=sock.makefile("rwb",buffering=0)
        if "QMP" not in json.loads(io.readline()):
            raise RuntimeError("QMP greeting missing")
        for command in (
            {"execute":"qmp_capabilities"},
            {"execute":"input-send-event","arguments":{"events":events}},
        ):
            io.write((json.dumps(command)+"\n").encode())
            for _ in range(40):
                response=json.loads(io.readline())
                if "error" in response:
                    raise RuntimeError("QMP rejected real input: "+str(response["error"]))
                if "return" in response:
                    break
            else:
                raise RuntimeError("no QMP reply")

def main()->int:
    p=argparse.ArgumentParser()
    p.add_argument("--serial",type=Path,required=True)
    p.add_argument("--qmp",type=Path,required=True)
    p.add_argument("--timeout",type=float,default=950)
    args=p.parse_args()
    deadline=time.monotonic()+args.timeout
    # Prior focus proof has already exercised independent A/B hit targets.
    # Wait for client frame 3 KMS ACK; the client can now terminate, but the
    # compositor must keep the retained window images alive.
    wait_marker(args.serial,"MELANIA_NATIVE_ASYNC_FRAME_COMPLETED=3",deadline)
    pointer(args.qmp,760,200)
    time.sleep(.15)
    pointer(args.qmp,760,200,"press")
    wait_marker(args.serial,"MELANIA_NATIVE_WINDOW_INTERACTION_BEGIN=2",deadline)
    pointer(args.qmp,850,280)
    wait_marker(args.serial,"MELANIA_NATIVE_DRAG_FRAME_SUBMITTED=4",deadline)
    # Keep the left button held. This second move arrives while KMS still
    # presents frame4. No frame5 may be scheduled until the verified ACK4.
    pointer(args.qmp,910,320)
    time.sleep(.2)
    pointer(args.qmp,910,320,"release")
    wait_marker(args.serial,"MELANIA_NATIVE_WINDOW_DRAG_MODEL=2 x=800 y=310",deadline)
    wait_marker(args.serial,"MELANIA_NATIVE_DRAG_FRAME_ACKED=4",deadline)
    wait_marker(args.serial,"MELANIA_NATIVE_DRAG_FRAME_SUBMITTED=5",deadline)
    wait_marker(args.serial,"MELANIA_NATIVE_DRAG_FRAME_ACKED=5",deadline)
    print("MELANIA_QEMU_REAL_POINTER_DRAG_TO_KMS=true",flush=True)
    print("MELANIA_QEMU_COALESCED_SECOND_POINTER_MOVE_PASS=true",flush=True)
    return 0

if __name__=="__main__":
    try:
        raise SystemExit(main())
    except (OSError,ValueError,RuntimeError,TimeoutError,EOFError) as error:
        print("MELANIA_QEMU_REAL_POINTER_DRAG_TO_KMS=false: "+str(error),file=sys.stderr)
        raise SystemExit(1)
