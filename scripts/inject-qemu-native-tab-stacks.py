#!/usr/bin/env python3
"""Genuine USB -> libinput -> Smithay tab stack -> wl_shm -> DRM CRTC proof.

This tests the QEMU-only native XDG tab implementation with independent
physical USB key events, actual wl_keyboard Enter and captured GPU output.
A text marker or Rust state-model result alone never satisfies the test.
"""
from __future__ import annotations
import argparse
import json
import re
import socket
import sys
import time
from pathlib import Path

START = "MELANIA_QEMU_REAL_TAB_STACK_STAGE_STARTED=true"
EXIT = "MELANIA_QEMU_NATIVE_GUEST_EXIT="
W, H = 1280, 800
BG = (27, 33, 58)
A = (217, 191, 78)  # Actual XRGB8888 client buffer at KMS frame 11
B = (34, 222, 121)  # Actual XRGB8888 client buffer at KMS frame 12

def stage(serial: Path, deadline: float) -> str:
    while time.monotonic() < deadline:
        content = serial.read_text(errors="replace") if serial.exists() else ""
        if START in content:
            return content[content.rfind(START):]
        if EXIT in content:
            raise RuntimeError("QEMU exited before tab-stack stage")
        time.sleep(.075)
    raise TimeoutError("native tab QEMU stage never started")

def wait(serial: Path, marker: str, deadline: float) -> None:
    while time.monotonic() < deadline:
        body = stage(serial, deadline)
        if marker in body:
            return
        if EXIT in body:
            raise RuntimeError("guest exited before genuine tab proof: " + marker)
        time.sleep(.065)
    raise TimeoutError("missing genuine Wayland/KMS tab marker: " + marker)

def qmp(path: Path, request: dict) -> None:
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.settimeout(8)
        sock.connect(str(path))
        stream = sock.makefile("rwb", buffering=0)
        if "QMP" not in json.loads(stream.readline()):
            raise RuntimeError("invalid QMP greeting")
        for cmd in ({"execute": "qmp_capabilities"}, request):
            stream.write((json.dumps(cmd) + "\n").encode())
            for _ in range(100):
                response = json.loads(stream.readline())
                if "error" in response:
                    raise RuntimeError("QMP command refused: " + str(response["error"]))
                if "return" in response:
                    break
            else:
                raise RuntimeError("QMP command lacks completion ACK")

def key(qmp_path: Path, code: str, down: bool) -> None:
    qmp(qmp_path, {"execute": "input-send-event", "arguments": {
        "events": [{"type": "key", "data": {
            "key": {"type": "qcode", "data": code}, "down": down,
        }}],
    }})
    time.sleep(.11)

def chord(qmp_path: Path, code: str, shift: bool = False) -> None:
    key(qmp_path, "meta_l", True)
    if shift:
        key(qmp_path, "shift", True)
    key(qmp_path, code, True)
    key(qmp_path, code, False)
    if shift:
        key(qmp_path, "shift", False)
    key(qmp_path, "meta_l", False)

def rgb(path: Path, name: str, x: int, y: int, expected: tuple[int,int,int]) -> None:
    data = path.read_bytes()
    m = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", data)
    if not m or tuple(map(int, m.groups())) != (W, H):
        raise ValueError("QEMU screendump is not a 1280x800 P6 image")
    offset = m.end()
    if len(data) != offset + 3*W*H:
        raise ValueError("truncated or padded physical CRTC image")
    start = offset + 3*(y*W+x)
    observed = tuple(data[start:start+3])
    if any(abs(actual-wanted) > 18 for actual,wanted in zip(observed, expected)):
        raise ValueError(f"{name} at {x},{y}: pixel {observed} != expected {expected}")

def frame(qmp_path:Path, target:Path,
          samples:tuple[tuple[str,int,int,tuple[int,int,int]], ...]) -> None:
    target.parent.mkdir(parents=True,exist_ok=True)
    last_error:Exception|None = None
    for _ in range(24):
        qmp(qmp_path, {"execute": "screendump", "arguments": {
            "filename": str(target.resolve()), "format": "ppm",
        }})
        try:
            for name,x,y,expected in samples:
                rgb(target,name,x,y,expected)
            return
        except (OSError,ValueError) as e:
            last_error = e
            time.sleep(.13)
    raise RuntimeError("real CRTC pixel content never matched: " + str(last_error))

def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--serial", type=Path, required=True)
    p.add_argument("--qmp", type=Path, required=True)
    p.add_argument("--out-dir", type=Path, required=True)
    p.add_argument("--timeout", type=float, default=1450)
    args = p.parse_args()
    deadline = time.monotonic() + args.timeout
    wait(args.serial, "MELANIA_QEMU_TAB_TWO_WAYLAND_SURFACES_READY=true", deadline)
    wait(args.serial, "MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=12", deadline)
    frame(args.qmp,args.out_dir/"tab-stack-before.ppm",(
        ("A original", 200,200,A), ("B original",740,260,B),
        ("background",40,40,BG),
    ))
    chord(args.qmp,"s")
    wait(args.serial,"MELANIA_NATIVE_TAB_STACKING=true id=1",deadline)
    chord(args.qmp,"s",shift=True)
    wait(args.serial,"MELANIA_NATIVE_TAB_ATTACHED=1:2",deadline)
    wait(args.serial,"MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=13",deadline)
    frame(args.qmp,args.out_dir/"tab-stack-attached-A.ppm",(
        ("A old location cleared",200,200,BG),
        ("A moved over B",740,260,A),
        ("background",40,40,BG),
    ))
    print("MELANIA_QEMU_TAB_REAL_CRTC_ATTACHED_A=true",flush=True)
    chord(args.qmp,"pgup",shift=True)
    wait(args.serial,"MELANIA_NATIVE_TAB_REORDERED=1 index=0",deadline)
    print("MELANIA_QEMU_TAB_REAL_REORDER_LEFT=true",flush=True)
    chord(args.qmp,"left")
    wait(args.serial,"MELANIA_NATIVE_TAB_ACTIVE=2",deadline)
    wait(args.serial,"MELANIA_QEMU_TAB_REAL_WL_KEYBOARD_ENTER=B",deadline)
    wait(args.serial,"MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=14",deadline)
    frame(args.qmp,args.out_dir/"tab-stack-B-active.ppm",(
        ("B selected image",740,260,B),
        ("A original still hidden",200,200,BG),
    ))
    print("MELANIA_QEMU_TAB_REAL_CRTC_SELECTED_B=true",flush=True)
    chord(args.qmp,"right")
    wait(args.serial,"MELANIA_NATIVE_TAB_ACTIVE=1",deadline)
    wait(args.serial,"MELANIA_QEMU_TAB_REAL_WL_KEYBOARD_ENTER=A",deadline)
    wait(args.serial,"MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=15",deadline)
    frame(args.qmp,args.out_dir/"tab-stack-A-restored.ppm",(
        ("A selected image",740,260,A),
        ("B retained but hidden",200,200,BG),
    ))
    print("MELANIA_QEMU_TAB_REAL_CRTC_SELECTED_A=true",flush=True)
    chord(args.qmp,"pgdn",shift=True)
    wait(args.serial,"MELANIA_NATIVE_TAB_REORDERED=1 index=1",deadline)
    print("MELANIA_QEMU_TAB_REAL_REORDER_RIGHT=true",flush=True)
    chord(args.qmp,"s")
    wait(args.serial,"MELANIA_NATIVE_TAB_STACKING=false id=1",deadline)
    wait(args.serial,"MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=16",deadline)
    frame(args.qmp,args.out_dir/"tab-stack-dissolved.ppm",(
        ("A original position restored",200,200,A),
        ("B original position restored",740,260,B),
        ("background",40,40,BG),
    ))
    print("MELANIA_QEMU_TAB_REAL_CRTC_DISSOLVED=true",flush=True)
    key(args.qmp,"f8",True)
    wait(args.serial,"MELANIA_QEMU_TAB_REAL_USB_RELEASE=F8",deadline)
    key(args.qmp,"f8",False)
    print("MELANIA_QEMU_TAB_REAL_USB_XDG_FOCUS_CRTC_PIXELS=true",flush=True)
    return 0

if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError,RuntimeError,TimeoutError,ValueError,EOFError) as error:
        print("MELANIA_QEMU_TAB_REAL_USB_XDG_FOCUS_CRTC_PIXELS=false: "+str(error),
              file=sys.stderr,flush=True)
        raise SystemExit(1)
