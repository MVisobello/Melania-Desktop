"""Stage62: partial frame baselines must be DRM-ACKed, not merely submitted."""
from pathlib import Path
import unittest
S=(Path(__file__).resolve().parents[1]/"melania-native/src/wayland_bridge.rs").read_text()
class NativePartialFrameSafety(unittest.TestCase):
    def test_only_confirmed_images_become_partial_baseline(self):
        self.assertIn("dynamic_confirmed_frame:Option<Vec<u8>>",S)
        self.assertIn("dynamic_submitted_frame:Option<Vec<u8>>",S)
        self.assertIn("self.dynamic_scene.composite_damaged_into(&mut next)",S)
        self.assertIn("self.dynamic_confirmed_frame=self.dynamic_submitted_frame.take()",S)
        self.assertLess(S.index("if !self.continuous_redraw.presented(planned)"),S.index("self.dynamic_confirmed_frame=self.dynamic_submitted_frame.take()"))
        self.assertIn("self.dynamic_submitted_frame=None;",S)
        self.assertIn("self.dynamic_confirmed_frame=None;",S)
    def test_fixed_qemu_frame_path_unchanged(self):
        self.assertIn("self.multi_scene.composite().map_err",S)
        self.assertIn("if self.dynamic_window_mode",S)
if __name__=="__main__":unittest.main()
