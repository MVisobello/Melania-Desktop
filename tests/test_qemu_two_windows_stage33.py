#!/usr/bin/env python3
"""Stage33: real independent xdg surfaces compose in the same KMS output.

Python checks the actual QMP PPM pixel validator and protocol wiring.
The Linux workflow also runs real Rust pixel tests and the guest/QMP proof.
"""
import importlib.util
from pathlib import Path
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[1]
SCENE=(ROOT/"melania-native/src/multi_window_scene.rs").read_text()
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-client-window.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
KMS=(ROOT/"melania-native/src/bin/qemu-rust-kms-stream.rs").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()
capture_path=ROOT/"scripts/capture-qemu-two-windows.py"
spec=importlib.util.spec_from_file_location("two_windows_capture",capture_path)
capture=importlib.util.module_from_spec(spec)
spec.loader.exec_module(capture)

W,H=1280,800

def frame(phase):
    pixels=bytearray([27,33,58] * (W*H))
    def fill(x,y,w,h,color):
        rgb=bytes(color)
        for row in range(y,y+h):
            for col in range(x,x+w):
                offset=(row*W+col)*3
                pixels[offset:offset+3]=rgb
    fill(154,114,372,236,(65,171,216))
    fill(160,120,360,224,(245,108,34))
    fill(340,120,180,224,(16,205,155))
    if phase>=2:
        fill(644,184,372,236,(65,171,216))
        fill(650,190,360,224,(73,126,237))
        fill(830,190,180,224,(247,211,48))
    if phase==3:
        fill(160,120,360,224,(222,64,118))
        fill(340,120,180,224,(117,238,90))
    return f"P6\n{W} {H}\n255\n".encode()+bytes(pixels)

class TwoWindowStage33(unittest.TestCase):
    def test_qmp_samples_both_windows_across_three_real_phases(self):
        pictures=[frame(index) for index in (1,2,3)]
        for index,contents in enumerate(pictures,1):
            actual=capture.validate(contents,index)
            self.assertEqual(actual["background"],(27,33,58))
            if index==2:
                self.assertEqual(actual["leftA"],(245,108,34))
                self.assertEqual(actual["leftB"],(73,126,237))
            if index==3:
                self.assertEqual(actual["leftA"],(222,64,118))
                self.assertEqual(actual["leftB"],(73,126,237))
        self.assertEqual(len(set(pictures)),3)
        with self.assertRaises(ValueError):
            capture.validate(pictures[1],1)

    def test_real_xdg_wl_shm_surfaces_are_distinct_and_retained(self):
        for needle in (
            "MELANIA_NATIVE_TWO_WINDOWS",
            "top_other=xdg_other.get_toplevel(&qh,())",
            "let surface=if two_windows && frame_index==1",
            "surface.attach(Some(&buffer),0,0)",
            "MELANIA_NATIVE_CLIENT_FRAME_WINDOW_ID=",
        ):
            self.assertIn(needle,CLIENT)
        for needle in (
            "known_window_surfaces:WindowSlots<WlSurface>",
            "self.known_window_surfaces.register(surface.clone())",
            "native_client_frame::copy_client_shm(ptr,len,metadata)",
            "self.multi_scene.upsert(key,imported)",
            "self.multi_scene.composite()",
            "MELANIA_NATIVE_TWO_REAL_WINDOWS_COMPOSITED=true",
        ):
            self.assertIn(needle,BRIDGE)
        self.assertIn("two_independent_client_surfaces_remain_visible_after_redraw",SCENE)
        self.assertIn("TooManyWindows",SCENE)
        self.assertIn("WINDOW_BYTES",SCENE)

    def test_proof_is_bound_to_actual_gpu_and_not_to_synthetic_images(self):
        for needle in (
            "MELANIA_QEMU_TWO_WINDOWS=YES_QEMU_VM",
            "MELANIA_NATIVE_TWO_WINDOWS=YES_QEMU_VM",
            "MELANIA_QEMU_TWO_REAL_WINDOWS_COMPOSITED_PASS=true",
            "MELANIA_RUST_KMS_RESTORE_VERIFIED=true",
        ):
            self.assertIn(needle,GUEST)
        self.assertIn("MELANIA_RUST_MULTI_FRAME{}_ACTIVE=true",KMS)
        self.assertIn("capture-qemu-two-windows.py",HOST)
        self.assertIn("MELANIA_QEMU_TWO_WINDOWS_THREE_CAPTURE_PASS=true",HOST)
        self.assertIn("MELANIA_QEMU_TWO_WINDOWS_FRAME3_PIXELS=true",HOST)
        self.assertIn("test_qemu_two_windows_stage33.py",CI)
        self.assertIn("cargo test -p melania-native --all-targets",CI)
        for script in ("scripts/run-qemu-melania-native.sh",
                       "scripts/melania-qemu-native-guest.sh"):
            r=subprocess.run(["bash","-n",str(ROOT/script)],
                capture_output=True,text=True,timeout=6)
            self.assertEqual(r.returncode,0,r.stderr)
        self.assertIn("-vga virtio",HOST)
        self.assertIn("accel=tcg",HOST)
        self.assertNotIn("vfio",HOST)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",GUEST)

if __name__=="__main__":
    unittest.main()
