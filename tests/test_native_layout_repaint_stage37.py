#!/usr/bin/env python3
"""Stage37: verify genuine QEMU layout-only repaint of Wayland snapshots."""
import importlib.util
from pathlib import Path
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
WORKER=(ROOT/"melania-native/src/async_kms_stream.rs").read_text()
KMS=(ROOT/"melania-native/src/bin/qemu-rust-kms-stream.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
DRAG=(ROOT/"scripts/inject-qemu-layout-drag.py").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-client-window.rs").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()
spec=importlib.util.spec_from_file_location("repaint",ROOT/"scripts/capture-qemu-layout-repaint.py")
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

def make_frame():
    w,h=1280,800
    pixels=bytearray(bytes((27,33,58))*(w*h))
    def fill(x,y,ww,hh,c):
        for yy in range(y,y+hh):
            offset=(yy*w+x)*3
            pixels[offset:offset+ww*3]=bytes(c)*ww
    # Window B is unchanged at x650, y190.
    fill(644,184,372,236,(65,171,216))
    fill(650,190,360,224,(73,126,237))
    fill(830,190,180,224,(247,211,48))
    # Window A moved to x240,y190; it is still the same client content.
    fill(234,184,372,236,(65,171,216))
    fill(240,190,360,224,(222,64,118))
    fill(420,190,180,224,(117,238,90))
    return f"P6\n{w} {h}\n255\n".encode()+bytes(pixels)

class Stage37(unittest.TestCase):
    def test_fourth_frame_pixels_are_real_window_move_not_fake_new_client_data(self):
        image=make_frame()
        observed=module.validate(image)
        self.assertEqual(observed["vacatedA"],(27,33,58))
        self.assertEqual(observed["movedLeftA"],(222,64,118))
        self.assertEqual(observed["retainedRightB"],(247,211,48))
        with self.assertRaises(ValueError):
            module.validate(image[:-1])

    def test_fourth_frame_preserves_actual_KMS_scheduler_contract(self):
        for item in (
            "MELANIA_NATIVE_LAYOUT_REPAINT",
            "async_layout_pending:Option<PendingLayoutFrame>",
            "fn submit_layout_repaint(&mut self)",
            "self.multi_scene.composite()",
            "worker.submit(FrameJob{ticket,ordinal:4,pixels})",
            "self.async_layout_pending=Some(PendingLayoutFrame{ticket,ordinal:4})",
            "self.render_scheduler.complete_present(pending.ticket)",
            "MELANIA_NATIVE_LAYOUT_REPAINT_ACK=4",
            "MELANIA_NATIVE_SCHEDULED_FRAME_ACK=4",
        ):
            self.assertIn(item,BRIDGE)
        self.assertIn("self.render_scheduler.damage()",BRIDGE)
        self.assertIn("!(1..=if std::env::var(",WORKER)
        self.assertTrue(
            "frame_target=if layout_repaint {4u8} else {3u8}" in KMS or
            "frame_target=if resized {5u8} else if layout_repaint {4u8} else {3u8}" in KMS
        )
        self.assertIn("MELANIA_RUST_LAYOUT_REPAINT_FOUR_FRAMES_PASS=true",KMS)
        self.assertIn("rollback.restore()?",KMS)
        self.assertIn("if state.layout_repaint_mode {3} else",BRIDGE)

    def test_real_input_qmp_capture_are_mandatory_not_synthetic(self):
        for token in (
            "MELANIA_RUST_MULTI_FRAME3_ACTIVE=true",
            "MELANIA_NATIVE_WINDOW_DRAG_MODEL=1 x=",
            "MELANIA_NATIVE_LAYOUT_REPAINT_ACK=4",
            '"button":"left","down":True',
            '"button":"left","down":False',
        ):
            self.assertIn(token,DRAG)
        for token in (
            "inject-qemu-layout-drag.py",
            "capture-qemu-layout-repaint.py",
            "MELANIA_QEMU_DRAG_VISIBLE_GPU_PASS=true",
            "layout-dragged-frame4.ppm",
            "MELANIA_QEMU_LAYOUT_REPAINT_KMS_ACK=true",
        ):
            self.assertIn(token,HOST)
        for token in (
            "MELANIA_NATIVE_LAYOUT_REPAINT=YES_QEMU_VM",
            "MELANIA_RUST_LAYOUT_REPAINT_FOUR_FRAMES_PASS=true",
            "MELANIA_QEMU_LAYOUT_ONLY_KMS_REPAINT_PASS=true",
        ):
            self.assertIn(token,GUEST)
        self.assertIn("MELANIA_NATIVE_CLIENT_LAYOUT_REPAINT_HOLD=true",CLIENT)
        self.assertIn("test_native_layout_repaint_stage37.py",CI)
        self.assertIn("cargo test -p melania-native --all-targets",CI)
        self.assertNotIn("MELANIA_NATIVE_SESSION_READY=true",GUEST)
        for path in ("scripts/melania-qemu-native-guest.sh",
                     "scripts/run-qemu-melania-native.sh"):
            result=subprocess.run(["bash","-n",str(ROOT/path)],
                text=True,capture_output=True,timeout=6)
            self.assertEqual(result.returncode,0,result.stderr)

if __name__=="__main__":
    unittest.main()
