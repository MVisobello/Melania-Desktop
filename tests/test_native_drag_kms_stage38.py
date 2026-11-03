#!/usr/bin/env python3
"""Stage38: fail-closed structural and real QEMU pixel verification gates."""
import importlib.util
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
def load(path:str)->str:
    return (ROOT/path).read_text()
BRIDGE=load("melania-native/src/wayland_bridge.rs")
STREAM=load("melania-native/src/bin/qemu-rust-kms-stream.rs")
ASYNC=load("melania-native/src/async_kms_stream.rs")
SCENE=load("melania-native/src/multi_window_scene.rs")
GUEST=load("scripts/melania-qemu-native-guest.sh")
HOST=load("scripts/run-qemu-melania-native.sh")
INJECT=load("scripts/inject-qemu-window-drag.py")
CAPTURE=load("scripts/capture-qemu-drag-frame.py")
CI=load(".github/workflows/qemu-melania-native.yml")

class NativeDragKmsStage38(unittest.TestCase):
    def test_kms_fourth_frame_is_real_compositor_damage(self):
        for token in (
            "fn submit_drag_frame(&mut self)",
            "self.multi_scene.composite()",
            "self.multi_scene.layout_generation()==self.presented_layout_generation",
            "self.render_scheduler.begin_frame()",
            "worker.submit(FrameJob{ticket,ordinal:4,pixels:frame})",
            "MELANIA_NATIVE_DRAG_FRAME_SUBMITTED=4",
            "MELANIA_NATIVE_DRAG_FRAME_ACKED=4",
            "layout_generation:self.multi_scene.layout_generation()",
            "self.presented_layout_generation=pending.layout_generation",
            "if let Some((surface,buffer))=pending.client",
            "if state.drag_redraw_mode {4} else {3}",
            "MELANIA_NATIVE_DRAG_FRAME_VISIBLE=true",
        ): self.assertIn(token,BRIDGE)
        self.assertIn("self.submit_drag_frame()",BRIDGE)
        self.assertIn("layout_generation()",SCENE)
        self.assertIn("!(1..=5).contains(&job.ordinal)",ASYNC)
        self.assertIn("MELANIA_RUST_DRAG_FRAME_ACTIVE=true",STREAM)
        self.assertIn("MELANIA_RUST_COMPOSITOR_DRAG_FRAME4_PASS=true",STREAM)
        self.assertIn("rollback.restore()",STREAM)

    def test_real_input_and_actual_gpu_pixels_required(self):
        self.assertIn('MELANIA_NATIVE_ASYNC_FRAME_COMPLETED=3',INJECT)
        self.assertIn('MELANIA_NATIVE_WINDOW_INTERACTION_BEGIN=2',INJECT)
        self.assertIn('MELANIA_NATIVE_DRAG_FRAME_SUBMITTED=4',INJECT)
        self.assertIn('pointer(args.qmp,760,200,"press")',INJECT)
        self.assertIn('pointer(args.qmp,850,280)',INJECT)
        self.assertIn('MELANIA_RUST_DRAG_FRAME_ACTIVE=true',CAPTURE)
        self.assertIn('"execute":"screendump"',CAPTURE)
        self.assertIn('"window_B_old_cleared"',CAPTURE)
        self.assertIn('"window_B_moved_left"',CAPTURE)
        self.assertIn('"window_B_moved_right"',CAPTURE)
        self.assertIn('MELANIA_QEMU_DRAG_FRAME4_PIXELS_VERIFIED=true',HOST)
        self.assertIn('MELANIA_QEMU_REAL_POINTER_DRAG_TO_KMS=true',HOST)
        self.assertIn('MELANIA_NATIVE_DRAG_REDRAW=YES_QEMU_VM',GUEST)
        self.assertIn('MELANIA_QEMU_DRAG_FRAME=YES_QEMU_VM',GUEST)
        self.assertIn('scripts/capture-qemu-drag-frame.py',CI)
        self.assertIn('scripts/inject-qemu-window-drag.py',CI)

    def test_cpu_side_ppm_validator_rejects_unmoved_window(self):
        spec=importlib.util.spec_from_file_location(
            "qemu_drag",ROOT/"scripts/capture-qemu-drag-frame.py"
        )
        module=importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        header=b"P6\n1280 800\n255\n"
        rgb=bytearray([27,33,58])*(1280*800)
        def paint(x,y,color):
            offset=(y*1280+x)*3
            rgb[offset:offset+3]=bytes(color)
        for (x,y),color in [
            ((250,232),(222,64,118)),
            ((737,270),(65,171,216)),
            ((820,380),(73,126,237)),
            ((1000,380),(247,211,48)),
        ]:
            paint(x,y,color)
        frame=header+bytes(rgb)
        self.assertEqual(module.verify(frame)["window_B_moved_left"],(73,126,237))
        paint(700,250,(73,126,237))
        with self.assertRaisesRegex(ValueError,"window_B_old_cleared"):
            module.verify(header+bytes(rgb))
        with self.assertRaises(ValueError):
            module.verify(header+bytes(rgb[:100]))

    def test_native_login_remains_disabled(self):
        self.assertIn('MELANIA_NATIVE_SESSION_READY=false',GUEST)
        self.assertIn('MELANIA_NATIVE_CONTINUOUS_COMPOSITOR_READY=false',GUEST)
        self.assertNotIn('MELANIA_NATIVE_SESSION_READY=true',GUEST)
        self.assertIn('YES_DISPOSABLE_QEMU',STREAM)

if __name__=="__main__":
    unittest.main()
