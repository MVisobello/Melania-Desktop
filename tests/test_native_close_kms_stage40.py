"""Stage40: actual fifth QEMU DRM frame must erase the closed xdg window.

Rust tests validate the bounded scheduler and retained window snapshots;
CI also runs a disposable QEMU guest and obtains a real GPU screendump.
This Python test checks the framebuffer verifier on synthetic inputs so an
incorrect screenshot cannot be reported as a successful fifth frame.
"""
from pathlib import Path
import importlib.util
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-client-window.rs").read_text()
WORKER=(ROOT/"melania-native/src/async_kms_stream.rs").read_text()
KMS=(ROOT/"melania-native/src/bin/qemu-rust-kms-stream.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
CAPTURE=ROOT/"scripts/capture-qemu-close-frame.py"


class NativeCloseKmsStage40(unittest.TestCase):
    def test_bounded_fifth_frame_is_gated_by_actual_client_destruction(self):
        for marker in (
            'close_redraw_mode:bool', 'close_frame_presented:bool',
            'MELANIA_NATIVE_CLOSE_DAMAGE_QUEUED=true',
            'fn submit_close_frame(&mut self)',
            'self.multi_scene.window_count()!=1',
            'self.render_scheduler.needs_redraw()',
            'worker.submit(FrameJob{ticket,ordinal:5,pixels:frame})',
            'self.async_pending=Some(PendingAsyncFrame{',
            'MELANIA_NATIVE_CLOSE_FRAME_SUBMITTED=5',
            'MELANIA_NATIVE_CLOSE_FRAME_ACKED=5',
            'MELANIA_NATIVE_CLOSE_FRAME_CALLBACK_DONE=true',
            'MELANIA_NATIVE_CLOSE_FRAME_VISIBLE=true',
            'self.close_frame_presented=true;',
            'self.multi_scene.remove(key)',
        ):
            self.assertIn(marker,BRIDGE)
        self.assertIn('!(1..=6).contains(&job.ordinal)',WORKER)
        self.assertIn('MELANIA_RUST_CLOSE_FRAME5_ACTIVE=true',KMS)
        self.assertIn('MELANIA_RUST_CLOSE_FRAME5_PASS=true',KMS)
        self.assertIn('MELANIA_NATIVE_CLIENT_FIRST_WINDOW_DESTROYED=true',CLIENT)
        self.assertIn('MELANIA_NATIVE_CLIENT_SECOND_WINDOW_HELD=true',CLIENT)
        self.assertIn('MELANIA_NATIVE_CLIENT_CLOSE_CALLBACK_AFTER_KMS=true',CLIENT)
        self.assertIn('let _close_callback=survivor.frame(&qh,());',CLIENT)
        self.assertIn('while state.frame_done_count<4 {',CLIENT)
        self.assertIn('complete_presented_frame(&survivor,5*16)',BRIDGE)
        self.assertIn('MELANIA_NATIVE_CLOSE_REDRAW=YES_QEMU_VM',GUEST)
        self.assertIn('MELANIA_QEMU_CLOSE_FRAME=YES_QEMU_VM',GUEST)
        self.assertIn('scripts/capture-qemu-close-frame.py',HOST)
        self.assertIn('MELANIA_QEMU_CLOSE_FRAME5_PIXELS_VERIFIED=true',HOST)
        self.assertIn('MELANIA_VM_REAL_WINDOW_CLOSE_FIFTH_KMS_FRAME=true',HOST)
        # Neither host login nor ordinary nested use is authorized.
        self.assertIn('MELANIA_NATIVE_SESSION_READY=false',GUEST)
        self.assertNotIn('MELANIA_NATIVE_SESSION_READY=true',GUEST)

    def test_fifth_gpu_screenshot_rejects_stale_first_window_pixels(self):
        spec=importlib.util.spec_from_file_location('close_kms',CAPTURE)
        module=importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        w,h=module.W,module.H
        header=f"P6\n{w} {h}\n255\n".encode()
        pixels=bytearray([27,33,58])*(w*h)
        def paint(x,y,rgb):
            start=(y*w+x)*3
            pixels[start:start+3]=bytes(rgb)
        # B moved on stage38 to (740,270), A is now gone.
        for (x,y),rgb in (
            ((40,40),(27,33,58)),
            ((250,232),(27,33,58)),
            ((700,250),(27,33,58)),
            ((737,270),(65,171,216)),
            ((820,380),(73,126,237)),
            ((1000,380),(247,211,48)),
        ):
            paint(x,y,rgb)
        valid=header+bytes(pixels)
        self.assertEqual(module.verify(valid)["window_A_removed"],(27,33,58))
        paint(250,232,(222,64,118))  # stale A would still be visible
        with self.assertRaisesRegex(ValueError,"window_A_removed"):
            module.verify(header+bytes(pixels))
        with self.assertRaises(ValueError):
            module.verify(header+bytes(pixels[:-7]))


if __name__ == '__main__':
    unittest.main()
