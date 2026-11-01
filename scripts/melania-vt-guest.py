#!/usr/bin/env python3
"""Guest-only, no-DRM-writes QEMU virtual-console round-trip validation.

Requires actual /dev/tty0 and QEMU DMI. Does not open or modify a DRM card.
The surrounding disposable VM test captures serial output as evidence.
"""
import fcntl
import os
import pathlib
import stat
import struct
import sys
import time

VT_GETSTATE = 0x5603
VT_ACTIVATE = 0x5606
VT_WAITACTIVE = 0x5607


def dmi_is_qemu():
    names = []
    for file in ("/sys/class/dmi/id/sys_vendor", "/sys/class/dmi/id/product_name"):
        try:
            names.append(pathlib.Path(file).read_text().strip().lower())
        except OSError:
            pass
    return any(word in value for value in names for word in ("qemu", "bochs", "kvm"))


def active_vt(fd):
    state = bytearray(6)
    fcntl.ioctl(fd, VT_GETSTATE, state, True)
    return struct.unpack_from("H", state)[0]


def switch_vt(fd, number):
    fcntl.ioctl(fd, VT_ACTIVATE, number)
    fcntl.ioctl(fd, VT_WAITACTIVE, number)
    got = active_vt(fd)
    if got != number:
        raise RuntimeError(f"requested VT{number}, found VT{got}")


def drm_card_present():
    return any(stat.S_ISCHR(path.stat().st_mode)
               for path in pathlib.Path("/dev/dri").glob("card[0-9]*"))


def main():
    if not dmi_is_qemu():
        raise RuntimeError("QEMU DMI hardware gate failed")
    if not drm_card_present():
        raise RuntimeError("QEMU virtual DRM card unavailable")
    print("MELANIA_QEMU_DRM_ENUMERATED=true", flush=True)
    fd = os.open("/dev/tty0", os.O_RDONLY | os.O_CLOEXEC)
    try:
        original = active_vt(fd)
        if not 1 <= original <= 63:
            raise RuntimeError(f"invalid original VT {original}")
        target = 2 if original != 2 else 3
        print(f"MELANIA_QEMU_VT_BEFORE={original}", flush=True)
        print(f"MELANIA_QEMU_VT_TARGET={target}", flush=True)
        try:
            switch_vt(fd, target)
            print("MELANIA_QEMU_VT_SWITCH=true", flush=True)
            time.sleep(1)
        finally:
            # Try restoring VT regardless of a failed activation/validation.
            switch_vt(fd, original)
        if active_vt(fd) != original:
            raise RuntimeError("original VT could not be restored")
        print(f"MELANIA_QEMU_VT_RESTORED={original}", flush=True)
        print("MELANIA_QEMU_VT_PASS=true", flush=True)
    finally:
        os.close(fd)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"MELANIA_QEMU_VT_FAIL={type(exc).__name__}: {exc}", flush=True)
        sys.exit(1)
