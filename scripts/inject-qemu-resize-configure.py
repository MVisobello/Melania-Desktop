#!/usr/bin/env python3
"""Stage38: QEMU-only real resize gesture and xdg.configure/ack proof.

Uses a disposable QEMU tablet, never host input. The window pixels are
still 360x224; this tests the actual protocol negotiation, not a GPU
resize or a new-size wl_shm import.
"""
import argparse
import json
from pathlib import Path
import socket
import sys
import time

def wait(serial:Path,marker:str,deadline:float)->None:
    while time.monotonic()<deadline:
        contents=serial.read_text(errors="replace") if serial.exists() else ""
        if marker in contents: return
        if "MELANIA_QEMU_NATIVE_GUEST_EXIT=" in contents:
            raise RuntimeError(f"VM exited before {marker}")
        time.sleep(.07)
    raise TimeoutError(f"real Wayland marker missing: {marker}")

def send(qmp:Path)->None:
    def point(x:int,y:int):
        return [
            {"type":"abs","data":{"axis":"x","value":round(x*32767/1279)}},
            {"type":"abs","data":{"axis":"y","value":round(y*32767/799)}},
        ]
    def btn(down:bool):
        return [{"type":"btn","data":{"button":"left","down":down}}]
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as sock:
        sock.settimeout(6)
        sock.connect(str(qmp))
        stream=sock.makefile("rwb",buffering=0)
        if "QMP" not in json.loads(stream.readline()):
            raise RuntimeError("not a QEMU QMP endpoint")
        def request(value:dict)->None:
            stream.write((json.dumps(value)+"\n").encode())
            for _ in range(40):
                response=json.loads(stream.readline())
                if "error" in response:
                    raise RuntimeError(f"QMP input error: {response['error']}")
                if "return" in response:return
            raise RuntimeError("QMP request not acknowledged")
        request({"execute":"qmp_capabilities"})
        # A moved +80,+70: origin (240,190), size 360x224.
        # The bottom-right 16-pixel corner starts resize.
        for label,events in (
            ("CORNER",point(594,408)),
            ("PRESS",btn(True)),
            ("RESIZE",point(620,430)),
            ("RELEASE",btn(False)),
        ):
            request({"execute":"input-send-event","arguments":{"events":events}})
            print(f"MELANIA_QEMU_RESIZE_{label}_SENT=true",flush=True)
            time.sleep(.16)

def main()->int:
    p=argparse.ArgumentParser()
    p.add_argument("--serial",required=True,type=Path)
    p.add_argument("--qmp",required=True,type=Path)
    p.add_argument("--timeout",type=float,default=950)
    a=p.parse_args()
    deadline=time.monotonic()+a.timeout
    wait(a.serial,"MELANIA_RUST_MULTI_FRAME4_ACTIVE=true",deadline)
    send(a.qmp)
    wait(a.serial,"MELANIA_NATIVE_RESIZE_CONFIGURE_SENT=1",deadline)
    wait(a.serial,"MELANIA_NATIVE_CLIENT_RESIZE_CONFIGURE_RECEIVED=",deadline)
    wait(a.serial,"MELANIA_NATIVE_CLIENT_RESIZE_CONFIGURE_ACK=",deadline)
    print("MELANIA_QEMU_REAL_XDG_RESIZE_ACK_PASS=true",flush=True)
    return 0

if __name__=="__main__":
    try:raise SystemExit(main())
    except (OSError,ValueError,RuntimeError,TimeoutError,EOFError) as error:
        print(f"MELANIA_QEMU_REAL_XDG_RESIZE_ACK_PASS=false: {error}",file=sys.stderr)
        raise SystemExit(1)
