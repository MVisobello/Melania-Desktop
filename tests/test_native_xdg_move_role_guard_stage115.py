"""Stage115: deny interactive XDG move after non-normal role transitions.

A source-contract regression only; actual QEMU and Linux CI must also pass.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
BRIDGE = (ROOT / "melania-native/src/wayland_bridge.rs").read_text()


class NativeXdgMoveRoleGuard(unittest.TestCase):
    def test_move_requires_normal_managed_window_after_consuming_serial(self):
        start = BRIDGE.index("fn move_request(&mut self,top:ToplevelSurface")
        end = BRIDGE.index("fn resize_request(", start)
        handler = BRIDGE[start:end]
        for marker in (
            "self.seat.owns(&seat)",
            "self.lifecycle.seat_active()",
            "self.native_presentation_guard.permanently_revoked()",
            "self.focused.as_ref()!=Some(surface)",
            "take_xdg_pointer_press(&mut self.last_xdg_press,surface,serial)",
            "self.managed_windows.iter().any(",
            "role==surface && window.state()==WindowDisplayState::Normal",
            "self.dynamic_scene.geometry(key)",
            "InteractionKind::Move",
        ):
            self.assertIn(marker, handler)
        self.assertLess(handler.index("take_xdg_pointer_press("),
                        handler.index("window.state()==WindowDisplayState::Normal"))
        self.assertLess(handler.index("window.state()==WindowDisplayState::Normal"),
                        handler.index("self.window_interaction.begin("))

    def test_resize_and_move_apply_same_role_policy(self):
        move = BRIDGE[BRIDGE.index("fn move_request("):BRIDGE.index("fn resize_request(")]
        resize = BRIDGE[BRIDGE.index("fn resize_request("):BRIDGE.index("fn maximize_request(")]
        policy = "role==surface && window.state()==WindowDisplayState::Normal"
        self.assertIn(policy, move)
        self.assertIn(policy, resize)


if __name__ == "__main__":
    unittest.main()
