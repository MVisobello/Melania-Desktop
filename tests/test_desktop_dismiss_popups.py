"""Regression guard for desktop click dismissing transient panels.

The compositor must only request dismissal when a click is outside
Overlay/Top layers, and the panel must keep GTK layer-shell roles alive.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class DesktopDismissTest(unittest.TestCase):
    def test_compositor_sends_dismiss_only_for_outside_click(self):
        source = (ROOT / "melania-compositor/src/main.rs").read_text()
        event = source.split("InputEvent::PointerButton { event, .. } => {", 1)[1]
        outside = event.split("if !layer_intercepts {", 1)[1]
        self.assertIn('socket.send_to(b"CLOSE_OVERLAYS",&destination)', outside)
        self.assertIn("map.layer_under(WlrLayer::Overlay", event)
        self.assertIn("map.layer_under(WlrLayer::Top", event)

    def test_panel_conceals_all_four_surfaces_without_unmapping(self):
        source = (ROOT / "melania-panel/src/main.rs").read_text()
        handler = source.split('Ok(n) if &packet[..n]==b"CLOSE_OVERLAYS" => {', 1)[1].split("},", 1)[0]
        for name, helper in (
            ("open", "hide_start_surface"),
            ("quick", "hide_popup_surface"),
            ("notifications", "hide_popup_surface"),
            ("calendar", "hide_popup_surface"),
        ):
            self.assertIn(f"{helper}(&{name});", handler)
        self.assertNotIn("set_visible(false)", handler)
        self.assertNotIn(".close()", handler)


if __name__ == "__main__":
    unittest.main()
