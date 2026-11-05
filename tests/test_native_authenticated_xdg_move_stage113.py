"""Stage113: real Wayland xdg_toplevel.move from libinput-authenticated press."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()

class NativeXdgMoveGrant113(unittest.TestCase):
    def test_real_pointer_button_serial_is_forwarded_and_recorded_once(self):
        start=BRIDGE.index("InputEvent::PointerButton {event} => {")
        end=BRIDGE.index("state.delivered_buttons+=1;",start)
        button=BRIDGE[start:end]
        for piece in (
            "let button_serial=SERIAL_COUNTER.next_serial();",
            "event.button_code()==0x110",
            "event.state()==ButtonState::Pressed",
            "state.managed_xdg_mode && state.dynamic_window_mode",
            "state.pointer_target(point)",
            "state.last_xdg_press=None;",
            "serial:button_serial,time:event.time_msec()",
        ):
            self.assertIn(piece,button)
        self.assertLess(button.index("pointer.motion(state,target.clone()"),
                        button.index("let button_serial=SERIAL_COUNTER.next_serial()"))
        self.assertLess(button.index("let button_serial=SERIAL_COUNTER.next_serial()"),
                        button.index("pointer.button(state,&ButtonEvent{"))
        self.assertIn("!(state.managed_xdg_mode && state.dynamic_window_mode)",button)
        # wl_pointer.motion has its own serial; only the actual button
        # event must reuse the move-authorization serial.
        button_event=button[button.index("pointer.button(state,&ButtonEvent{"):]
        self.assertNotIn("serial:SERIAL_COUNTER.next_serial(),time:event.time_msec()",
                         button_event)

    def test_xdg_client_move_reuses_real_window_interaction_not_synthetic_repaint(self):
        start=BRIDGE.index("fn move_request(&mut self,top:ToplevelSurface")
        end=BRIDGE.index("fn resize_request(",start)
        move=BRIDGE[start:end]
        for piece in (
            "self.managed_xdg_mode",
            "self.dynamic_window_mode",
            "self.lifecycle.seat_active()",
            "self.native_presentation_guard.permanently_revoked()",
            "self.focused.as_ref()!=Some(surface)",
            "take_xdg_pointer_press(&mut self.last_xdg_press,surface,serial)",
            "self.known_window_surfaces.id_of(surface)",
            "self.dynamic_scene.geometry(key)",
            "self.window_interaction.begin(",
            "InteractionKind::Move",
        ):
            self.assertIn(piece,move)
        self.assertNotIn("native_presentation_guard.arm_initial",move)
        self.assertNotIn("continuous_redraw.presented(",move)

    def test_revocation_consumption_and_stale_press_are_not_reusable(self):
        self.assertIn("token.take().is_some_and",BRIDGE)
        self.assertIn("exact_real_press_serial_surface_and_one_shot_are_required",BRIDGE)
        self.assertIn("state.last_xdg_press=None;",BRIDGE)
        self.assertIn("fn toplevel_destroyed",BRIDGE)
        self.assertIn("state.window_interaction.cancel();",BRIDGE)
        self.assertIn("state.native_presentation_guard.revoke();",BRIDGE)

if __name__=="__main__":
    unittest.main()
