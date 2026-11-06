"""First real managed XDG buffer must acquire keyboard focus before Super Snap."""
from pathlib import Path
import unittest
BRIDGE=(Path(__file__).resolve().parents[1]/"melania-native/src/wayland_bridge.rs").read_text()
class RealManagedXdgFocus(unittest.TestCase):
    def test_focus_after_validated_client_buffer_not_role_configure(self):
        focus=BRIDGE.index("                // Real mapped XDG clients must receive keyboard focus")
        start=BRIDGE.rfind("match self.continuous_client_ledger.stage(surface.clone(),buffer)",0,focus)
        self.assertGreaterEqual(start,0)
        end=BRIDGE.index("self.note_scene_change();",focus)
        body=BRIDGE[start:end]
        for token in (
            "self.managed_xdg_mode && self.dynamic_window_mode",
            "self.focused.is_none()",
            "self.lifecycle.seat_active()",
            "self.xdg_shell.toplevel_surfaces().iter()",
            "top.wl_surface()==surface",
            "keyboard.set_focus(self,Some(surface.clone())",
            "self.focused=Some(surface.clone())",
            "MELANIA_NATIVE_FIRST_COMMITTED_XDG_FOCUSED=",
        ):
            self.assertIn(token,body)
        self.assertLess(body.index("match self.continuous_client_ledger.stage("),
                        body.index("keyboard.set_focus("))
    def test_initial_focus_is_exclusive_to_mapped_toplevels(self):
        self.assertNotIn("keyboard.set_focus(",BRIDGE[BRIDGE.index("fn new_toplevel("):
             BRIDGE.index("fn new_popup(")])
        self.assertIn("if state.focused.is_none() {return;}",BRIDGE)

if __name__=="__main__":
    unittest.main()
