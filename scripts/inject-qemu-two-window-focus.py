#!/usr/bin/env python3
"""QEMU-only pointer focus proof for actual independent Wayland windows.

Send absolute coordinates to the disposable VM USB tablet via its PRIVATE QMP
socket during Rust KMS frame holds. The server must deliver real libinput
pointer presses to the hit-tested wl_surface and switch keyboard focus:
while the renderer is still active: first window B, then window A. Never touch host input devices.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import socket
import sys
import time

MARKER2="MELANIA_RUST_MULTI_FRAME2_ACTIVE=true"
GUEST_EXIT="MELANIA_QEMU_NATIVE_GUEST_EXIT="

def wait_for(serial:Path,marker:str,end:float)->None:
    while time.monotonic()<end:
        data=serial.read_text(errors="replace") if serial.exists() else ""
        if marker in data:
            return
        if GUEST_EXIT in data:
            raise RuntimeError(f"QEMU exited before {marker}")
        time.sleep(.08)
    raise TimeoutError(f"missing guest serial marker: {marker}")

def send_click(qmp:Path,x:int,y:int)->None:
    if not (0<=x<1280 and 0<=y<800):
        raise ValueError("pointer outside virtual 1280x800 display")
    def ev(typ:str,data:dict)->dict:
        return {"type":typ,"data":data}
    events=[
        ev("abs",{"axis":"x","value":round(x*32767/1279)}),
        ev("abs",{"axis":"y","value":round(y*32767/799)}),
        ev("btn",{"button":"left","down":True}),
        ev("btn",{"button":"left","down":False}),
    ]
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as sock:
        sock.settimeout(5)
        sock.connect(str(qmp))
        stream=sock.makefile("rwb",buffering=0)
        greeting=json.loads(stream.readline())
        if "QMP" not in greeting:
            raise RuntimeError("QEMU QMP greeting missing")
        def execute(payload:dict)->None:
            stream.write((json.dumps(payload)+"\n").encode())
            for _ in range(40):
                reply=json.loads(stream.readline())
                if "error" in reply:
                    raise RuntimeError(f"QMP input rejected: {reply['error']}")
                if "return" in reply:
                    return
            raise RuntimeError("QMP input reply missing")
        execute({"execute":"qmp_capabilities"})
        # Mouse motion and button events MUST be distinct QMP dispatches;
        # otherwise two absolute coordinates and a click can be coalesced
        # by the tablet driver before libinput receives a movement event.
        for stage,part in (("MOVE",events[:2]),("PRESS",[events[2]]),("RELEASE",[events[3]])):
            execute({"execute":"input-send-event","arguments":{"events":part}})
            print(f"MELANIA_QEMU_WINDOW_QMP_{stage}=true x={x} y={y}",flush=True)
            time.sleep(.12)

def main()->int:
    p=argparse.ArgumentParser()
    p.add_argument("--serial",type=Path,required=True)
    p.add_argument("--qmp",type=Path,required=True)
    p.add_argument("--timeout",type=float,default=950)
    args=p.parse_args()
    end=time.monotonic()+args.timeout
    wait_for(args.serial,MARKER2,end)
    send_click(args.qmp,750,280)
    wait_for(args.serial,"MELANIA_NATIVE_WINDOW_POINTER_FOCUS=2",end)
    print("MELANIA_QEMU_WINDOW2_POINTER_FOCUS=true",flush=True)
    # Both clicks must happen in the SAME still-active display epoch.
    # Waiting for frame 3 is racy: Smithay is allowed to close as soon
    # as the final post-KMS callback has been delivered.
    send_click(args.qmp,250,230)
    wait_for(args.serial,"MELANIA_NATIVE_WINDOW_POINTER_FOCUS=1",end)
    print("MELANIA_QEMU_WINDOW1_POINTER_FOCUS=true",flush=True)
    print("MELANIA_QEMU_TWO_WINDOW_REAL_INPUT_PASS=true",flush=True)
    return 0

if __name__=="__main__":
    try:
        raise SystemExit(main())
    except (OSError,ValueError,TimeoutError,RuntimeError,EOFError) as error:
        print(f"MELANIA_QEMU_TWO_WINDOW_REAL_INPUT_PASS=false: {error}",file=sys.stderr)
        raise SystemExit(1)
