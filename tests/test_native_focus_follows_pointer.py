"""Native QEMU pointer-focus audit; no production-login acceptance implied."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
BRIDGE = (ROOT / "melania-native/src/wayland_bridge.rs").read_text()
POLICY = (ROOT / "melania-native/src/focus_follows_pointer.rs").read_text()

class NativeFocusFollowsPointer(unittest.TestCase):
    def test_unsafe_guest_opt_in_is_not_default(self):
        self.assertIn('MELANIA_NATIVE_FOCUS_FOLLOWS_POINTER', BRIDGE)
        self.assertIn('Ok("YES_QEMU_VM")', BRIDGE)
        self.assertIn('MELANIA_NATIVE_MANAGED_XDG', BRIDGE)
        self.assertIn('MELANIA_NATIVE_DYNAMIC_SCENE', BRIDGE)
        self.assertIn('MELANIA_NATIVE_TWO_WINDOWS', BRIDGE)
        self.assertIn('MELANIA_NATIVE_SESSION_READY=false', BRIDGE)

    def test_both_real_libinput_motion_paths_forward_then_hover_focus(self):
        start = BRIDGE.index('InputEvent::PointerMotion {event} =>')
        end = BRIDGE.index('InputEvent::PointerAxis {event} =>', start)
        section = BRIDGE[start:end]
        self.assertEqual(section.count('state.maybe_focus_hovered_toplevel(point)'), 2)
        self.assertEqual(section.count('pointer.motion(state,target,'), 2)
        self.assertEqual(section.count('pointer.frame(state);'), 2)
        self.assertEqual(section.count('state.apply_window_motion(point);'), 2)

    def test_keyboard_focus_without_raise_or_fabricated_frames(self):
        start = BRIDGE.index('fn maybe_focus_hovered_toplevel')
        end = BRIDGE.index('fn apply_window_motion', start)
        section = BRIDGE[start:end]
        self.assertIn('self.pointer_target(point)', section)
        self.assertIn('self.xdg_shell.toplevel_surfaces()', section)
        self.assertIn('should_focus_on_motion(HoverFocusGate', section)
        self.assertIn('keyboard.set_focus(self,Some(surface.clone())', section)
        self.assertNotIn('dynamic_scene.raise(', section)
        self.assertNotIn('submit_continuous_frame', section)

    def test_revoke_held_buttons_on_device_and_seat_loss(self):
        self.assertIn('state.pointer_buttons_held.insert(event.button_code())', BRIDGE)
        self.assertIn('state.pointer_buttons_held.remove(&event.button_code())', BRIDGE)
        # Device removal clears buttons directly; both real VT pause and
        # udev GPU loss call the shared Rust revoker. Do not just count
        # duplicated text or miss a path after safe helper extraction.
        device=BRIDGE[BRIDGE.index("InputEvent::DeviceRemoved"):
                      BRIDGE.index("InputEvent::Keyboard {event}")]
        revoker=BRIDGE[BRIDGE.index("fn revoke_wayland_input_for_lost_seat("):
                       BRIDGE.index("/// Keep the original libseat-owned descriptor")]
        self.assertIn("state.pointer_buttons_held.clear();",device)
        self.assertIn("state.pointer_buttons_held.clear();",revoker)
        for start,end in (("SessionEvent::PauseSession=>{","SessionEvent::ActivateSession=>{"),
                          ("let reason=state.gpu_watch.observe(&change);",
                           "MELANIA_NATIVE_DRM_TOPOLOGY_REVOKED=")):
            block=BRIDGE[BRIDGE.index(start):BRIDGE.index(end,BRIDGE.index(start))]
            self.assertIn("revoke_wayland_input_for_lost_seat(state);",block)
        for item in ('!gate.button_held','!gate.interactive_grab',
                     '!gate.popup_grab','gate.visible_toplevel',
                     '!gate.revoked','gate.seat_active'):
            self.assertIn(item, POLICY)


if __name__=="__main__":
    unittest.main()
