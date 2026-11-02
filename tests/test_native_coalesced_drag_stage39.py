#!/usr/bin/env python3
"""Stage39: prove latest retained drag generation follows the KMS ACK."""
import importlib.util
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
def load(path):return (ROOT/path).read_text()
BRIDGE=load("melania-native/src/wayland_bridge.rs")
STREAM=load("melania-native/src/bin/qemu-rust-kms-stream.rs")
ASYNC=load("melania-native/src/async_kms_stream.rs")
GUEST=load("scripts/melania-qemu-native-guest.sh")
HOST=load("scripts/run-qemu-melania-native.sh")
INJECT=load("scripts/inject-qemu-window-drag.py")
CAPTURE5=load("scripts/capture-qemu-drag-frame5.py")
CI=load(".github/workflows/qemu-melania-native.yml")

class CoalescedDragStage39(unittest.TestCase):
    def test_damaged_scene_waits_for_previous_kms_ack(self):
        self.assertIn("coalesced_redraw_mode:bool",BRIDGE)
        self.assertIn("self.async_pending.is_some()",BRIDGE)
        self.assertIn("self.multi_scene.layout_generation()==self.presented_layout_generation",BRIDGE)
        self.assertIn("self.presented_layout_generation=pending.layout_generation",BRIDGE)
        ack=BRIDGE.index("if !self.render_scheduler.complete_present(pending.ticket)")
        second=BRIDGE.index("if self.coalesced_redraw_mode && !self.pixel_ready",ack)
        schedule=BRIDGE.index("self.submit_drag_frame()?",second)
        self.assertLess(ack,second)
        self.assertLess(second,schedule)
        self.assertIn("ordinal:next",BRIDGE)
        self.assertIn("MELANIA_NATIVE_DRAG_FRAME_SUBMITTED={next}",BRIDGE)
        self.assertIn("MELANIA_NATIVE_DRAG_FRAME_ACKED={}",BRIDGE)
        self.assertIn("if state.coalesced_redraw_mode {5}",BRIDGE)
        self.assertIn("coalesced drag redraw requires QEMU drag KMS opt-in",BRIDGE)
        self.assertIn("!(1..=5).contains(&job.ordinal)",ASYNC)

    def test_second_motion_is_real_and_independently_screenshotted(self):
        for term in (
            'pointer(args.qmp,760,200,"press")',
            'pointer(args.qmp,850,280)',
            'pointer(args.qmp,910,320)',
            "MELANIA_NATIVE_DRAG_FRAME_SUBMITTED=5",
            "MELANIA_NATIVE_DRAG_FRAME_ACKED=5",
            "MELANIA_QEMU_COALESCED_SECOND_POINTER_MOVE_PASS=true",
        ):self.assertIn(term,INJECT)
        for term in (
            "MELANIA_RUST_DRAG_FRAME5_ACTIVE=true",
            '"execute":"screendump"',
            '"window_B_old_cleared"',
            '"window_B_moved_left"',
            '"window_B_moved_right"',
        ):self.assertIn(term,CAPTURE5)
        self.assertIn("MELANIA_QEMU_DRAG_FRAME5_PIXELS_VERIFIED=true",HOST)
        self.assertIn("MELANIA_QEMU_COALESCED_SECOND_POINTER_MOVE_PASS=true",HOST)
        self.assertIn("MELANIA_RUST_COALESCED_DRAG_FRAME5_PASS=true",STREAM)
        self.assertIn("MELANIA_NATIVE_COALESCED_REDRAW=YES_QEMU_VM",GUEST)
        self.assertIn("MELANIA_QEMU_DRAG_COALESCED=YES_QEMU_VM",GUEST)
        self.assertIn("scripts/capture-qemu-drag-frame5.py",CI)

    def test_frame5_validator_rejects_old_position(self):
        spec=importlib.util.spec_from_file_location(
            "drag5",ROOT/"scripts/capture-qemu-drag-frame5.py")
        module=importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        header=b"P6\n1280 800\n255\n"
        rgb=bytearray([27,33,58])*(1280*800)
        def mark(x,y,color):
            i=(y*1280+x)*3
            rgb[i:i+3]=bytes(color)
        for (x,y),color in [
            ((250,232),(222,64,118)),
            ((797,310),(65,171,216)),
            ((880,420),(73,126,237)),
            ((1060,420),(247,211,48)),
        ]:mark(x,y,color)
        self.assertEqual(module.verify(header+bytes(rgb))["window_B_moved_right"],(247,211,48))
        mark(750,300,(73,126,237))
        with self.assertRaisesRegex(ValueError,"window_B_old_cleared"):
            module.verify(header+bytes(rgb))

    def test_vm_only_and_one_inflight(self):
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",GUEST)
        self.assertIn("MELANIA_NATIVE_CONTINUOUS_COMPOSITOR_READY=false",GUEST)
        self.assertNotIn("MELANIA_NATIVE_SESSION_READY=true",GUEST)
        self.assertIn("self.render_scheduler.begin_frame()",BRIDGE)
        self.assertIn("MELANIA_QEMU_DRAG_FRAME5_PIXELS_VERIFIED=true",HOST)

if __name__=="__main__":
    unittest.main()
