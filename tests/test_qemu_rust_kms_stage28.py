#!/usr/bin/env python3
"""Stage 28: Rust-only direct DRM scanout of TWO actual Wayland client commits.

Unit/static checks are not proof of successful KMS operations: the QEMU VM
must independently capture the first and second framebuffer via QMP and then
confirm CRTC restoration. All devices belong to the sacrificial QEMU guest.
"""
from pathlib import Path
import importlib.util
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[1]
KMS=(ROOT/"melania-native/src/bin/qemu-rust-kms-frames.rs").read_text()
WAYLAND=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
FRAME=(ROOT/"melania-native/src/native_client_frame.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-client-window.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()

spec=importlib.util.spec_from_file_location("rust_frames",
    ROOT/"scripts/capture-qemu-rust-kms-frames.py")
capture=importlib.util.module_from_spec(spec)
spec.loader.exec_module(capture)

def ppm(frame:int)->bytes:
    w,h=1280,800
    image=bytearray(bytes((27,33,58))*(w*h))
    def at(x,y,rgb):
        pos=(y*w+x)*3
        image[pos:pos+3]=bytes(rgb)
    at(157,120,(65,171,216))
    if frame==1:
        at(250,232,(245,108,34))
        at(430,232,(16,205,155))
    else:
        at(250,232,(73,126,237))
        at(430,232,(247,211,48))
    return f"P6\n{w} {h}\n255\n".encode()+image

class RustKmsTwoFrames(unittest.TestCase):
    def test_qmp_verifier_rejects_interchanged_or_blank_frames(self):
        one=ppm(1);two=ppm(2)
        self.assertNotEqual(one,two)
        self.assertEqual(capture.validate(one,1)["left"],(245,108,34))
        self.assertEqual(capture.validate(two,2)["right"],(247,211,48))
        with self.assertRaises(ValueError):capture.validate(one,2)
        with self.assertRaises(ValueError):capture.validate(two,1)
        with self.assertRaises(ValueError):capture.validate(b"P6 1 1 255 \0\0\0",1)

    def test_separate_client_commits_and_smithay_buffer_releases(self):
        for needle in (
            "MELANIA_NATIVE_TWO_FRAMES",
            "for frame_index in 0..count",
            "surface.attach(Some(&buffer)",
            "surface.commit()",
            "MELANIA_NATIVE_CLIENT_FRAME_COMMITTED={}",
        ):
            self.assertIn(needle,CLIENT)
        for needle in (
            "MELANIA_NATIVE_CLIENT_UPDATES",
            "self.pixel_frame_count<self.pixel_frame_target",
            "native_client_frame::write_frame_index(&frame",
            "buffer.release()",
            "MELANIA_NATIVE_CLIENT_FRAME_COUNT={}",
        ):
            self.assertIn(needle,WAYLAND)
        self.assertIn("FRAME2_PATH",FRAME)
        self.assertIn("write_frame_index(frame:&[u8],index:u8)",FRAME)
        # Compositor sources never substitute hardcoded client colors.
        source=FRAME.split("#[cfg(test)]")[0]
        self.assertNotIn("(73,126,237)",source)
        self.assertNotIn("(247,211,48)",source)

    def test_all_kms_calls_are_rust_and_original_crtc_is_restored(self):
        for needle in (
            'MELANIA_QEMU_RUST_TWO_FRAME_KMS',
            'YES_DISPOSABLE_QEMU',
            'Path::new("/dev/tty1")',
            "fs::read_to_string(\"/sys/class/dmi/id/sys_vendor\")",
            "DrmFourcc::Xrgb8888",
            "card.create_dumb_buffer(",
            "card.map_dumb_buffer(",
            "card.add_framebuffer(",
            "card.set_crtc(",
            "card.page_flip(",
            "wait_for_fb(",
            "MELANIA_RUST_KMS_FRAME1_ACTIVE=true",
            "MELANIA_RUST_KMS_FRAME2_ACTIVE=true",
            "MELANIA_RUST_KMS_RESTORE_VERIFIED={}",
            "MELANIA_RUST_KMS_TWO_CLIENT_FRAMES_PASS=true",
            "MELANIA_NATIVE_SESSION_READY=false",
        ):
            self.assertIn(needle,KMS)
        self.assertIn("impl Drop for Restore",KMS)
        self.assertIn("if self.submitted && !self.restored",KMS)
        self.assertIn("&& observed.mode()==old.mode()",KMS)
        self.assertNotIn('Command::new("melania-qemu-client-window-scanout")',KMS)

    def test_host_gates_two_actual_qmp_frames_and_vm_only_execution(self):
        for needle in (
            "melania-rust-kms",
            "MELANIA_NATIVE_CLIENT_UPDATES=YES_QEMU_VM",
            "MELANIA_NATIVE_TWO_FRAMES=YES_QEMU_VM",
            "MELANIA_QEMU_RUST_TWO_FRAME_KMS=YES_DISPOSABLE_QEMU",
            "MELANIA_QEMU_NATIVE_RUST_DIRECT_KMS_PASS=true",
            "MELANIA_QEMU_RUST_KMS_TWO_CAPTURE_PASS=true",
            "MELANIA_RUST_KMS_RESTORE_VERIFIED=true",
            "MELANIA_RUST_KMS_FRAME1_ACTIVE=true",
            "MELANIA_RUST_KMS_FRAME2_ACTIVE=true",
            "MELANIA_VM_RUST_DIRECT_KMS_TWO_CLIENT_FRAMES=true",
            "accel=tcg",
            "-vga virtio",
        ):
            self.assertIn(needle,GUEST+HOST)
        self.assertNotIn("accel=kvm",HOST)
        self.assertNotIn("vfio",HOST)
        self.assertIn("qemu-rust-kms-frames",CI)
        self.assertIn("test_qemu_rust_kms_stage28.py",CI)
        self.assertIn("MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false",HOST)

    def test_fail_closed_without_vm_and_shell_syntax(self):
        result=subprocess.run(["bash","scripts/run-qemu-melania-native.sh"],
            cwd=ROOT,env={"PATH":"/usr/bin:/bin"},
            capture_output=True,text=True,timeout=8)
        self.assertEqual(result.returncode,78,result.stderr)
        for path in ("scripts/run-qemu-melania-native.sh",
                     "scripts/melania-qemu-native-guest.sh"):
            result=subprocess.run(["bash","-n",str(ROOT/path)],
                capture_output=True,text=True,timeout=8)
            self.assertEqual(result.returncode,0,result.stderr)

if __name__=="__main__":
    unittest.main()
