"""Partial frame baselines must be DRM-ACKed, never merely submitted."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
S=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
WORKER=(ROOT/"melania-native/src/continuous_kms_stream.rs").read_text()

class NativePartialFrameSafety(unittest.TestCase):
    def test_only_confirmed_images_become_partial_baseline(self):
        self.assertIn("dynamic_confirmed_frame:Option<Vec<u8>>",S)
        self.assertNotIn("dynamic_submitted_frame",S)
        self.assertIn("self.dynamic_confirmed_frame.take()",S)
        self.assertIn("self.dynamic_scene.composite_damaged_into(&mut previous)",S)
        self.assertIn("worker.submit(ContinuousKmsJob{planned,pixels})",S)
        self.assertIn("pub pixels: Vec<u8>",WORKER)
        self.assertIn("planned:job.planned,status,pixels:job.pixels",WORKER)
        confirmed="self.dynamic_confirmed_frame=Some(reply.pixels)"
        self.assertIn(confirmed,S)
        self.assertLess(S.index("if !self.continuous_redraw.presented(planned)"),S.index(confirmed))
        self.assertLess(S.index("if let Err(error)=reply.status"),S.index(confirmed))
        self.assertIn("self.dynamic_confirmed_frame=None;",S)

    def test_fixed_qemu_frame_path_unchanged(self):
        self.assertIn("self.multi_scene.composite().map_err",S)
        self.assertIn("if self.dynamic_window_mode",S)

if __name__=="__main__":unittest.main()
