#!/usr/bin/env python3
"""Verify and export the exact native explicit-sync outcome from guest serial."""
from collections import Counter
from pathlib import Path
import sys

KERNEL_OK = "MELANIA_QEMU_REAL_DRM_SYNCOBJ_TIMELINE_EVENTFD_PASS=true"
NO_EVENT = "MELANIA_QEMU_DRM_SYNCOBJ_EVENTFD_UNSUPPORTED=true"
NO_TIMELINE = "MELANIA_QEMU_DRM_SYNCOBJ_TIMELINE_UNSUPPORTED=true"
WIRE_OK = "MELANIA_QEMU_REAL_WAYLAND_EXPLICIT_SYNC_TWO_GPU_FRAMES_PASS=true"
WIRE_SKIP = "MELANIA_QEMU_WAYLAND_EXPLICIT_SYNC_UNSUPPORTED=true"
FRAMES = tuple(f"MELANIA_QEMU_EXPLICIT_WIRE_FRAME_VERIFIED={i}" for i in (1, 2))


def verify(serial: str) -> str:
    seen = Counter(line.strip() for line in serial.splitlines())
    if sum(seen[x] for x in (KERNEL_OK, NO_EVENT, NO_TIMELINE)) != 1:
        raise ValueError("missing, duplicated or conflicting DRM capability result")
    if seen[KERNEL_OK]:
        if seen[WIRE_OK] != 1 or seen[WIRE_SKIP] or any(seen[x] != 1 for x in FRAMES):
            raise ValueError("supported DRM explicit sync did not verify both real wire frames")
        return "MELANIA_VM_EXPLICIT_SYNC_RESULT=VERIFIED_TWO_GPU_FRAMES"
    if seen[WIRE_SKIP] != 1 or seen[WIRE_OK] or any(seen[x] for x in FRAMES):
        raise ValueError("inconsistent unsupported explicit sync result")
    return ("MELANIA_VM_EXPLICIT_SYNC_RESULT=UNSUPPORTED_EVENTFD"
            if seen[NO_EVENT] else "MELANIA_VM_EXPLICIT_SYNC_RESULT=UNSUPPORTED_TIMELINE")


if __name__ == "__main__":
    try:
        if len(sys.argv) != 2:
            raise ValueError("expected one guest serial log")
        print(verify(Path(sys.argv[1]).read_text(errors="replace")))
    except (OSError, ValueError) as error:
        print(f"Explicit-sync evidence rejected: {error}", file=sys.stderr)
        sys.exit(1)
