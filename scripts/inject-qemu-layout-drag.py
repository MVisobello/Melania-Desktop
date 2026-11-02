#!/usr/bin/env python3
"""Stage37: move a real Wayland window using ONLY QEMU's virtual tablet.

The signal is the fourth KMS ACK generated from retained client wl_shm
pixels after moving window A. A single QMP connection sends actual virtual
USB movement, left button press, drag and release. No host mouse access.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import socket
import sys
import time

def wait_marker(serial:Path,marker:str,end:float)->None:
    while time.monotonic()<end:
        data=serial.read_text(errors="replace") if serial.exists() else ""
        if marker in data:
            return
        if "MELANIA_QEMU_NATIVE_GUEST_EXIT=" in data:
            raise RuntimeError(f"guest exited without {marker}")
        time.sleep(.08)
    raise TimeoutError(f"missing real-guest marker {marker}")

def drag(qmp:Path)->None:
    def pointer(x:int,y:int)->list[dict]:
        return [
            {"type":"abs","data":{"axis":"x","value":round(x*32767/1279)}},
            {"type":"abs","data":{"axis":"y","value":round(y*32767/799)}},
        ]
    press=[{"type":"btn","data":{"button":"left","down":True}}]
    release=[{"type":"btn","data":{"button":"left","down":False}}]
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as sock:
        sock.settimeout(6)
        sock.connect(str(qmp))
        stream=sock.makefile("rwb",buffering=0)
        if "QMP" not in json.loads(stream.readline()):
            raise RuntimeError("not a QEMU QMP socket")
        def command(payload:dict)->None:
            stream.write((json.dumps(payload)+"\n").encode())
            for _ in range(40):
                result=json.loads(stream.readline())
                if "error" in result:
                    raise RuntimeError(f"QMP rejected virtual input: {result['error']}")
                if "return" in result:
                    return
            raise RuntimeError("QMP response missing")
        command({"execute":"qmp_capabilities"})
        for label,events in (
            ("HOVER",pointer(300,129)),
            ("PRESS",press),
            ("MOVE",pointer(380,199)),
            ("RELEASE",release),
        ):
            command({"execute":"input-send-event","arguments":{"events":events}})
            print(f"MELANIA_QEMU_DRAG_{label}_SENT=true",flush=True)
            time.sleep(.16)

def main()->int:
    p=argparse.ArgumentParser()
    p.add_argument("--serial",required=True,type=Path)
    p.add_argument("--qmp",required=True,type=Path)
    p.add_argument("--timeout",type=float,default=950)
    args=p.parse_args()
    deadline=time.monotonic()+args.timeout
    wait_marker(args.serial,"MELANIA_RUST_MULTI_FRAME3_ACTIVE=true",deadline)
    drag(args.qmp)
    wait_marker(args.serial,"MELANIA_NATIVE_WINDOW_DRAG_MODEL=1 x=",deadline)
    print("MELANIA_QEMU_REAL_MOUSE_DRAG_PROCESSED=true",flush=True)
    wait_marker(args.serial,"MELANIA_NATIVE_LAYOUT_REPAINT_ACK=4",deadline)
    print("MELANIA_QEMU_LAYOUT_REPAINT_KMS_ACK=true",flush=True)
    return 0

if __name__=="__main__":
    try:
        raise SystemExit(main())
    except (OSError,RuntimeError,TimeoutError,ValueError,EOFError) as error:
        print(f"MELANIA_QEMU_LAYOUT_REPAINT_KMS_ACK=false: {error}",file=sys.stderr)
        raise SystemExit(1)
