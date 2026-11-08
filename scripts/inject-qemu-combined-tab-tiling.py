#!/usr/bin/env python3
"""Stage136: real USB → Smithay XDG group → two owned shm → actual GPU CRTC.

Reuses the independently tested QMP/PPM parser from Stage129, never
synthesizes Wayland serials, pixels, focus, ACKs, or DRM presentation.
"""
from __future__ import annotations
import argparse
import importlib.util
import sys
import time
from pathlib import Path

p=Path(__file__).with_name("inject-qemu-native-tab-stacks.py")
spec=importlib.util.spec_from_file_location("melania_native_tab_qmp",p)
if spec is None or spec.loader is None:
    raise RuntimeError("missing independent native tab QMP verifier")
import types
tab=types.ModuleType(spec.name)
sys.modules[spec.name]=tab
spec.loader.exec_module(tab)
tab.START="MELANIA_QEMU_COMBINED_TAB_STAGE_STARTED=true"

def main()->int:
    args=argparse.ArgumentParser()
    args.add_argument("--serial",type=Path,required=True)
    args.add_argument("--qmp",type=Path,required=True)
    args.add_argument("--out-dir",type=Path,required=True)
    args.add_argument("--timeout",type=float,default=1900)
    a=args.parse_args()
    deadline=time.monotonic()+a.timeout
    tab.wait(a.serial,"MELANIA_QEMU_COMBINED_TAB_TWO_WAYLAND_SURFACES_READY=true",deadline)
    tab.wait(a.serial,"MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=12",deadline)
    tab.frame(a.qmp,a.out_dir/"combined-tabs-before.ppm",(
        ("A original",200,200,tab.A),
        ("B original",740,260,tab.B),
        ("untouched wallpaper",40,40,tab.BG),
    ))
    tab.chord(a.qmp,"s")
    tab.wait(a.serial,"MELANIA_NATIVE_TAB_STACKING=true id=1",deadline)
    tab.chord(a.qmp,"s",shift=True)
    tab.wait(a.serial,"MELANIA_NATIVE_TAB_ATTACHED=1:2",deadline)
    # Only this REAL guest key enables the initially disabled tiling model.
    tab.chord(a.qmp,"y")
    tab.wait(a.serial,"MELANIA_NATIVE_AUTO_TILING_ENABLED=true",deadline)
    tab.wait(a.serial,"MELANIA_NATIVE_COMBINED_TILING_ARMED=1 clients=2",deadline)
    tab.wait(a.serial,"MELANIA_QEMU_COMBINED_TAB_TWO_REAL_XDG_ACKS=true",deadline)
    tab.wait(a.serial,"MELANIA_QEMU_COMBINED_TAB_TWO_OWNED_SHM_CALLBACKS=true",deadline)
    tab.wait(a.serial,"MELANIA_NATIVE_TAB_RESIZE_ATOMIC_SCENE_READY=1 clients=2",deadline)
    tab.wait(a.serial,"MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=14",deadline)
    # Frame13 was an earlier genuine attach; 14 is the atomic A/B resize.
    # Both 1280x800 images exist, but exactly active tab A may be painted.
    tab.frame(a.qmp,a.out_dir/"combined-tabs-active-A.ppm",(
        ("A full-screen client",200,200,tab.A),
        ("A at previous B location",740,260,tab.A),
        ("A at output corner",40,40,tab.A),
    ))
    print("MELANIA_QEMU_COMBINED_TAB_ATOMIC_CRTC_A=true",flush=True)
    tab.chord(a.qmp,"left")
    tab.wait(a.serial,"MELANIA_NATIVE_TAB_ACTIVE=2",deadline)
    tab.wait(a.serial,"MELANIA_QEMU_TAB_REAL_WL_KEYBOARD_ENTER=B",deadline)
    tab.wait(a.serial,"MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=15",deadline)
    tab.frame(a.qmp,a.out_dir/"combined-tabs-active-B.ppm",(
        ("B full-screen client",200,200,tab.B),
        ("B at previous B location",740,260,tab.B),
        ("B at output corner",40,40,tab.B),
    ))
    print("MELANIA_QEMU_COMBINED_TAB_REAL_CRTC_B=true",flush=True)
    tab.chord(a.qmp,"right")
    tab.wait(a.serial,"MELANIA_NATIVE_TAB_ACTIVE=1",deadline)
    tab.wait(a.serial,"MELANIA_QEMU_TAB_REAL_WL_KEYBOARD_ENTER=A",deadline)
    tab.wait(a.serial,"MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=16",deadline)
    tab.frame(a.qmp,a.out_dir/"combined-tabs-active-A-return.ppm",(
        ("A restored full-screen",200,200,tab.A),
        ("B buffer concealed",740,260,tab.A),
    ))
    print("MELANIA_QEMU_COMBINED_TAB_REAL_CRTC_A_RESTORED=true",flush=True)
    tab.key(a.qmp,"f8",True)
    tab.wait(a.serial,"MELANIA_QEMU_TAB_REAL_USB_RELEASE=F8",deadline)
    tab.key(a.qmp,"f8",False)
    print("MELANIA_QEMU_COMBINED_TAB_REAL_XDG_SHM_KMS_PASS=true",flush=True)
    return 0

if __name__=="__main__":
    try:
        raise SystemExit(main())
    except (OSError,RuntimeError,TimeoutError,ValueError,EOFError) as error:
        print("MELANIA_QEMU_COMBINED_TAB_REAL_XDG_SHM_KMS_PASS=false: "+
              str(error),file=sys.stderr,flush=True)
        raise SystemExit(1)
