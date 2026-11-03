"""Stage59: never retire framebuffer damage from attempted or stale KMS flips."""
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
SCENE=(ROOT/"melania-native/src/dynamic_window_scene.rs").read_text()
class NativeDamageAckTest(unittest.TestCase):
    def test_damage_ack_follows_verified_presentation(self):
        ack=BRIDGE.index("if !self.continuous_redraw.presented(planned)")
        retire=BRIDGE.index("self.dynamic_scene.confirm_presented_damage(planned.generation)")
        count=BRIDGE.index("self.continuous_frame_count=self.continuous_frame_count.saturating_add(1)")
        self.assertLess(ack,retire)
        self.assertLess(retire,count)
        self.assertIn("if ack_generation!=self.generation {return false;}",SCENE)
        self.assertIn("self.damage.clear_after_presented()",SCENE)
    def test_no_fake_native_login(self):
        self.assertNotIn("NATIVE_SESSION_READY=true",BRIDGE)
if __name__=="__main__":unittest.main()
