"""Regression: a revoked native seat must not expose stale selection sources.

Static wiring assertions complement the real Rust/Smithay CI build and the
isolated QEMU guest; they do not claim physical VT or GPU testing.
"""
from pathlib import Path
import unittest

BRIDGE = (Path(__file__).resolve().parents[1] /
          "melania-native/src/wayland_bridge.rs").read_text()


class SeatSelectionRevocation(unittest.TestCase):
    def test_both_selection_sources_are_cancelled_on_lost_seat(self):
        start = BRIDGE.index("fn revoke_wayland_input_for_lost_seat(")
        end = BRIDGE.index("\n}", start)
        body = BRIDGE[start:end]
        operations = [
            "revoke_wayland_focus_for_lost_seat(state);",
            "set_data_device_focus(&state.display_handle,&state.seat,None);",
            "clear_data_device_selection(&state.display_handle,&state.seat);",
            "set_primary_focus(&state.display_handle,&state.seat,None);",
            "clear_primary_selection(&state.display_handle,&state.seat);",
        ]
        positions = [body.index(op) for op in operations]
        self.assertEqual(positions, sorted(positions))

    def test_all_fatal_seat_paths_invoke_revocation(self):
        self.assertGreaterEqual(
            BRIDGE.count("revoke_wayland_input_for_lost_seat(state);"), 3
        )
        self.assertIn("revoke_wayland_input_for_lost_seat(&mut state);", BRIDGE)

    def test_focus_notifications_cannot_restore_revoked_offers(self):
        start = BRIDGE.index("fn focus_changed(&mut self,seat:&Seat<Self>")
        end = BRIDGE.index("\n    fn cursor_image(", start)
        body = BRIDGE[start:end]
        self.assertIn("self.lifecycle.seat_active()", body)
        self.assertIn(
            "!self.native_presentation_guard.permanently_revoked()", body
        )
        self.assertIn("set_data_device_focus(", body)
        self.assertIn("set_primary_focus(", body)


    def test_gtk_start_real_focus_respects_same_seat_epoch(self):
        # QEMU's special GTK Start keyboard-focus path must not bypass the
        # normal SeatHandler epoch guard after VT pause or fatal GPU loss.
        start = BRIDGE.index("fn refresh_qemu_gtk_start_focus(")
        end = BRIDGE.index("fn note_scene_change(", start) if "fn note_scene_change(" in BRIDGE[start:] else BRIDGE.index("fn submit_continuous_frame(", start)
        body = BRIDGE[start:end]
        self.assertIn("!self.lifecycle.seat_active()", body)
        self.assertIn("self.native_presentation_guard.permanently_revoked()", body)
        self.assertIn("keyboard.set_focus(self,Some(surface.clone())", body)
        self.assertLess(body.index("!self.lifecycle.seat_active()"),
                        body.index("keyboard.set_focus(self,Some(surface.clone())"))
        self.assertLess(body.index("self.native_presentation_guard.permanently_revoked()"),
                        body.index("keyboard.set_focus(self,Some(surface.clone())"))


if __name__ == "__main__":
    unittest.main()
