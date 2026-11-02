#!/usr/bin/env python3
"""Stage23: actual visible QEMU-only KMS scanout with QMP pixel evidence.

Static checks protect host isolation; synthetic PPM verifies the QMP
screenshot parser. Only the separate disposable VM runs the C DRM program.
"""
import importlib.util
from pathlib import Path
import subprocess
import sys
import unittest

ROOT=Path(__file__).resolve().parents[1]
C=(ROOT/"scripts/melania-qemu-visible-frame.c").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()
RUST=(ROOT/"melania-native/src/rollback_audit.rs").read_text()

spec=importlib.util.spec_from_file_location("qmp_capture",
    ROOT/"scripts/capture-qemu-visible-scanout.py")
capture=importlib.util.module_from_spec(spec)
spec.loader.exec_module(capture)

def ppm(w=640,h=480,left=(240,32,176),right=(16,232,224)):
    one=bytes(left)*(w//2)+bytes(right)*(w-w//2)
    return f"P6\n{w} {h}\n255\n".encode()+one*h

class QemuVisibleScanout(unittest.TestCase):
    def test_qmp_parser_recognizes_exact_real_guest_pattern(self):
        result=capture.validate_scanout(ppm())
        self.assertIn("MELANIA_QEMU_VISIBLE_PIXELS_VERIFIED=true",result)
        self.assertIn("640x480",result)

    def test_qmp_parser_rejects_blank_truncated_or_wrong_frame(self):
        for data in (b"",ppm()[:100],ppm(left=(0,0,0),right=(0,0,0)),
                     b"P6\n4000 2000\n255\n"+b"\0"*64):
            with self.subTest(size=len(data)),self.assertRaises(ValueError):
                capture.validate_scanout(data)

    def test_vm_only_probe_requires_hard_gates_and_original_fb(self):
        for needle in ("MELANIA_QEMU_VISIBLE_FRAME",
                       "YES_DISPOSABLE_QEMU","approved_guest()","is_qemu()",
                       '"/dev/tty"',"isatty(STDIN_FILENO)",'getenv("DISPLAY")',
                       'getenv("WAYLAND_DISPLAY")',
                       "resources->count_crtcs!=1",
                       "EXACTLY_ONE_ACTIVE_OUTPUT_REQUIRED",
                       "before->buffer_id",
                       "DRM_IOCTL_MODE_CREATE_DUMB","drmModeAddFB(",
                       "drmModeSetCrtc(","MELANIA_QEMU_VISIBLE_RESTORE_VERIFIED",
                       "same_crtc(before,original)","drmModeRmFB(",
                       "MELANIA_QEMU_VISIBLE_SCANOUT_ACTIVE=true"):
            # TTY uses strncmp rather than a string-literal path check.
            if needle=='"/dev/tty"':
                self.assertIn('strncmp(tty,"/dev/tty",8)',C)
            else:
                self.assertIn(needle,C)
        self.assertLess(C.index("drmModeSetCrtc("),
                        C.index("MELANIA_QEMU_VISIBLE_RESTORE_VERIFIED"))
        self.assertIn('if (!success || !restored)',C)

    def test_no_host_gpu_passthrough_and_real_screenshot_gate(self):
        for needle in ("accel=tcg","-vga virtio",
                       "sha256sum -c verify.sha256",
                       "capture-qemu-visible-scanout.py",
                       "melania-qmp.sock","MELANIA_QEMU_VISIBLE_PIXELS_VERIFIED=true",
                       "MELANIA_QEMU_VISIBLE_RESTORE_VERIFIED=true",
                       "MELANIA_QEMU_NATIVE_VISIBLE_EXPERIMENT_PASS=true",
                       "MELANIA_VM_ONLY_VISIBLE_FRAME_AND_CRTC_RESTORE=true"):
            self.assertIn(needle,HOST)
        for forbidden in ("accel=kvm","vfio","/dev/dri/card0"):
            self.assertNotIn(forbidden,HOST)
        self.assertIn("MELANIA_NATIVE_VISIBLE_SCANOUT=false",GUEST)
        self.assertIn("MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false",HOST)
        self.assertIn("visible_kms_commit_allowed(&self) -> bool {false}",RUST)
        self.assertIn("melania-qemu-visible-frame.c",CI)
        self.assertIn("test_qemu_visible_scanout.py",CI)

    def test_host_still_refuses_implicit_execution(self):
        run=subprocess.run(["bash",str(ROOT/"scripts/run-qemu-melania-native.sh")],
                           cwd=ROOT,env={"PATH":"/usr/bin:/bin"},
                           capture_output=True,text=True,timeout=5)
        self.assertEqual(run.returncode,78,run.stderr)
        self.assertIn("MELANIA_QEMU_NATIVE_HOST_BLOCKED",run.stderr)
        for path in ("scripts/run-qemu-melania-native.sh",
                     "scripts/melania-qemu-native-guest.sh"):
            result=subprocess.run(["bash","-n",str(ROOT/path)],
                                  capture_output=True,text=True,timeout=5)
            self.assertEqual(result.returncode,0,result.stderr)

if __name__=="__main__":
    unittest.main()
