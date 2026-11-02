#!/usr/bin/env python3
"""Stage 26: verify that QEMU displays pixels authored by a Wayland client.

This structural/unit test is not itself the graphical proof: the QEMU job
must run a real wl_shm xdg client, capture a live VM QMP screenshot, compare
window/interior/background pixels, and verify exact CRTC restoration.
"""
from pathlib import Path
import importlib.util
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[1]
RUST=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
FRAME=(ROOT/"melania-native/src/native_client_frame.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-client-window.rs").read_text()
C=(ROOT/"scripts/melania-qemu-client-window-scanout.c").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()
spec=importlib.util.spec_from_file_location("capture_client",
    ROOT/"scripts/capture-qemu-client-window.py")
capture=importlib.util.module_from_spec(spec)
spec.loader.exec_module(capture)

def ppm()->bytes:
    w,h=1280,800
    frame=bytearray([27,33,58])*(w*h)
    def setpixel(x,y,rgb):
        index=(y*w+x)*3
        frame[index:index+3]=bytes(rgb)
    setpixel(157,120,(65,171,216))
    setpixel(250,232,(245,108,34))
    setpixel(430,232,(16,205,155))
    return f"P6\n{w} {h}\n255\n".encode()+frame

class ClientPixelStage26(unittest.TestCase):
    def test_qmp_matches_four_distinct_pixel_sources(self):
        self.assertIn("MELANIA_QEMU_CLIENT_PIXELS_VERIFIED=true",
                      capture.validate_scanout(ppm()))
        bad=bytearray(ppm())
        bad[-1]=3
        self.assertRaises(ValueError,capture.validate_scanout,b"not a PPM")
        altered=bytearray(ppm())
        # Error in the real client window must fail even if the background
        # and border are exactly correct.
        start=altered.index(b"\n255\n")+5
        p=start+(232*1280+250)*3
        altered[p:p+3]=b"\0\0\0"
        self.assertRaises(ValueError,capture.validate_scanout,bytes(altered))

    def test_real_wayland_shm_not_synthetic_compositor_paint(self):
        for marker in ("wl_shm::Format::Xrgb8888",
                       "shm.create_pool(file.as_fd()",
                       "pool.create_buffer(",
                       "xdg.get_toplevel",
                       "surface.attach(Some(&buffer)",
                       "surface.damage_buffer(",
                       "MELANIA_NATIVE_CLIENT_WL_SHM_RELEASED=true",
                       "MELANIA_NATIVE_CLIENT_XDG_WINDOW_DRAWN=true"):
            self.assertIn(marker,CLIENT)
        for marker in ("with_buffer_contents(&buffer",
                       "native_client_frame::compose_from_shm",
                       "native_client_frame::write_frame",
                       "buffer.release()",
                       "ShmState::new::<BridgeState>",
                       "delegate_shm!(BridgeState)",
                       "MELANIA_NATIVE_CLIENT_FRAME_COMPOSED=true"):
            self.assertIn(marker,RUST)
        self.assertIn("std::ptr::read_volatile",FRAME)
        self.assertIn("format:wl_shm::Format::Xrgb8888",FRAME)
        self.assertIn("FRAME_PATH",FRAME)
        self.assertNotIn("245,108,34",FRAME.split("#[cfg(test)]")[0])
        self.assertNotIn("16,205,155",FRAME.split("#[cfg(test)]")[0])

    def test_vm_only_visible_scanout_restoration(self):
        for needle in ("approved_guest()","is_qemu()","isatty(STDIN_FILENO)",
                       "YES_DISPOSABLE_QEMU","O_NOFOLLOW",
                       '"/run/melania-native-qemu/client-frame.bgra"',
                       "pread(source,row,remaining,offset)",
                       "drmModeSetCrtc(","same_crtc(before,original)",
                       "MELANIA_QEMU_CLIENT_WINDOW_SCANOUT_ACTIVE=true",
                       "MELANIA_QEMU_CLIENT_WINDOW_RESTORE_VERIFIED",
                       "MELANIA_QEMU_CLIENT_WINDOW_RESULT=PASS"):
            self.assertIn(needle,C)
        self.assertIn("if (!success || !restored)",C)
        self.assertNotIn("Bright magenta",C)
        self.assertIn("MELANIA_NATIVE_CLIENT_PIXELS=YES_QEMU_VM",GUEST)
        self.assertIn("MELANIA_QEMU_CLIENT_WINDOW_FRAME=YES_DISPOSABLE_QEMU",GUEST)

    def test_host_enforces_real_client_and_screen_pixels(self):
        for needle in (
            "MELANIA_QEMU_CLIENT_PIXELS_VERIFIED=true",
            "MELANIA_NATIVE_CLIENT_WL_SHM_RELEASED=true",
            "MELANIA_NATIVE_CLIENT_FRAME_COMPOSED=true",
            "MELANIA_QEMU_CLIENT_WINDOW_RESTORE_VERIFIED=true",
            "MELANIA_QEMU_NATIVE_CLIENT_WINDOW_SCANOUT_PASS=true",
            "MELANIA_NATIVE_LIVE_KMS_RENDERING=false",
            "MELANIA_QEMU_NATIVE_POST_KMS_VT_PASS=true",
            "MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false",
            "accel=tcg","-vga virtio",
        ):
            self.assertIn(needle,HOST)
        self.assertNotIn("accel=kvm",HOST)
        self.assertNotIn("vfio",HOST)
        self.assertIn("qemu-client-window",CI)
        self.assertIn("melania-qemu-client-window-scanout.c",CI)

    def test_scripts_remain_fail_closed(self):
        for path in ("scripts/run-qemu-melania-native.sh",
                     "scripts/melania-qemu-native-guest.sh"):
            p=subprocess.run(["bash","-n",str(ROOT/path)],capture_output=True,
                             text=True,timeout=8)
            self.assertEqual(p.returncode,0,p.stderr)
        p=subprocess.run(["bash","scripts/run-qemu-melania-native.sh"],
                         cwd=ROOT,env={"PATH":"/usr/bin:/bin"},
                         capture_output=True,text=True,timeout=8)
        self.assertEqual(p.returncode,78,p.stderr)
        self.assertIn("MELANIA_QEMU_NATIVE_HOST_BLOCKED",p.stderr)

if __name__=="__main__":
    unittest.main()
