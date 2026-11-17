#!/usr/bin/env python3
"""Send one real QEMU USB tablet drag, only after retained role close ACK."""
import argparse
import runpy
import time
from pathlib import Path

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("--serial",type=Path,required=True)
    parser.add_argument("--qmp",type=Path,required=True)
    parser.add_argument("--timeout",type=float,required=True)
    args=parser.parse_args()
    helper=runpy.run_path(str(Path(__file__).with_name("inject-qemu-dynamic-pointer.py")))
    until=time.monotonic()+args.timeout
    helper["wait_marker"](args.serial,"MELANIA_QEMU_RETAINED_GPU_READY_FOR_REAL_DRAG=true",until)
    tablet=helper["tablet"]
    tablet(args.qmp,180,130)
    time.sleep(.15)
    tablet(args.qmp,180,130,"press")
    time.sleep(.15)
    tablet(args.qmp,430,165)
    # Do not release or send the next position until the real kernel ACK
    # for the intermediate position has been observed in guest serial.
    helper["wait_marker"](args.serial,"MELANIA_QEMU_RETAINED_GPU_HELD_DRAG_FRAME14=true",until)
    tablet(args.qmp,670,200)
    time.sleep(.15)
    tablet(args.qmp,670,200,"release")
    print("MELANIA_QEMU_RETAINED_GPU_USB_GESTURE_SENT=true",flush=True)
    keys=runpy.run_path(str(Path(__file__).with_name("inject-qemu-super-snap.py")))
    helper["wait_marker"](args.serial,"MELANIA_QEMU_RETAINED_GPU_READY_FOR_MINIMIZE=true",until)
    keys["chord"](args.qmp,"down")
    helper["wait_marker"](args.serial,"MELANIA_QEMU_RETAINED_GPU_MINIMIZED_FRAME16=true",until)
    keys["chord"](args.qmp,"up")
    print("MELANIA_QEMU_RETAINED_GPU_USB_VISIBILITY_SENT=true",flush=True)
    helper["wait_marker"](args.serial,"MELANIA_QEMU_RETAINED_GPU_RESTORED_FRAME17=true",until)
    # Second drag starts inside A's client area, not the compositor title band.
    tablet(args.qmp,700,240)
    time.sleep(.15)
    tablet(args.qmp,700,240,"press")
    helper["wait_marker"](args.serial,"MELANIA_QEMU_RETAINED_GPU_XDG_MOVE_GRANTED=true",until)
    tablet(args.qmp,700,400)
    time.sleep(.15)
    tablet(args.qmp,700,400,"release")
    helper["wait_marker"](args.serial,"MELANIA_QEMU_RETAINED_GPU_OVERLAP_FRAME18=true",until)
    # Focus already-topmost D first; this must not add a KMS frame.
    tablet(args.qmp,800,650)
    time.sleep(.15)
    tablet(args.qmp,800,650,"press")
    time.sleep(.15)
    tablet(args.qmp,800,650,"release")
    keys["key"](args.qmp,"f8",True)
    keys["key"](args.qmp,"f8",False)
    # Click the exposed upper part of A, not the overlapping D pixels.
    tablet(args.qmp,700,400)
    time.sleep(.15)
    tablet(args.qmp,700,400,"press")
    time.sleep(.15)
    tablet(args.qmp,700,400,"release")
    print("MELANIA_QEMU_RETAINED_GPU_USB_RAISE_SENT=true",flush=True)

if __name__=="__main__":
    main()
