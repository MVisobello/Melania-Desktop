#!/usr/bin/env python3
"""Send only virtual keyboard and pointer input to a disposable QEMU guest.

Never accesses the host keyboard/mouse, /dev/input or any host DRM card.
Waits for the real Wayland client to advertise readiness on the QEMU serial
log. The existing private QMP Unix socket belongs to the QEMU TCG process.
"""
import argparse
import json
from pathlib import Path
import socket
import sys
import time

def send_qmp(path:Path,commands:list[dict])->None:
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as conn:
        conn.settimeout(6)
        conn.connect(str(path))
        stream=conn.makefile("rwb",buffering=0)
        greeting=json.loads(stream.readline())
        if "QMP" not in greeting:
            raise RuntimeError("not a QEMU QMP endpoint")
        for cmd in [{"execute":"qmp_capabilities"},*commands]:
            stream.write((json.dumps(cmd)+"\n").encode())
            for _ in range(20):
                response=json.loads(stream.readline())
                if "error" in response:
                    raise RuntimeError(f"QEMU rejected virtual input: {response['error']}")
                if "return" in response:
                    break
            else:
                raise RuntimeError("QMP input reply missing")

def command(event_type:str,data:dict)->dict:
    return {"execute":"input-send-event",
            "arguments":{"events":[{"type":event_type,"data":data}]}}
def main()->int:
    parser=argparse.ArgumentParser()
    parser.add_argument("--serial",required=True,type=Path)
    parser.add_argument("--qmp",required=True,type=Path)
    parser.add_argument("--timeout",type=float,default=950)
    args=parser.parse_args()
    deadline=time.monotonic()+args.timeout
    while time.monotonic()<deadline:
        lines=args.serial.read_text(errors="replace") if args.serial.exists() else ""
        if "MELANIA_NATIVE_WAYLAND_CLIENT_READY=true" in lines:
            if not args.qmp.exists():
                raise RuntimeError("QMP Unix socket missing")
            events=[
                command("key",{"down":True,"key":{"type":"qcode","data":"a"}}),
                command("key",{"down":False,"key":{"type":"qcode","data":"a"}}),
                command("btn",{"down":True,"button":"left"}),
                command("btn",{"down":False,"button":"left"}),
            ]
            send_qmp(args.qmp,events)
            print("MELANIA_QEMU_NATIVE_USB_INPUT_INJECTED=true",flush=True)
            return 0
        if "MELANIA_QEMU_NATIVE_GUEST_EXIT=" in lines:
            raise RuntimeError("guest exited before native Wayland input probe")
        time.sleep(.25)
    raise TimeoutError("Wayland client readiness marker missing")

if __name__=="__main__":
    try:
        raise SystemExit(main())
    except (OSError,RuntimeError,TimeoutError,ValueError) as error:
        print(f"MELANIA_QEMU_NATIVE_USB_INPUT_INJECTED=false: {error}",file=sys.stderr)
        raise SystemExit(1)
