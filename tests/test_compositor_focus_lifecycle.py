"""Regression contracts for compositor keyboard focus on window lifecycle.

These source-level checks are complementary to the real Cargo and nested
Wayland tests in CI; they are not a substitute for interactive focus QA.
"""
import unittest
from pathlib import Path

SOURCE = (Path(__file__).resolve().parents[1] / "melania-compositor/src/main.rs").read_text()


class CompositorFocusLifecycle(unittest.TestCase):
    @staticmethod
    def part(start: str, end: str) -> str:
        return SOURCE.split(start, 1)[1].split(end, 1)[0]

    def test_dead_window_cannot_remain_previous_start_focus(self):
        layer = self.part("fn layer_destroyed(", "impl SeatHandler for Melania")
        start = self.part("fn refresh_start_focus(", "fn usable_area(")
        for section in (layer, start):
            self.assertIn("self.workspaces.is_visible(w.id)", section)
            self.assertIn("!w.minimized", section)

    def test_close_and_minimize_run_focus_cleanup_after_unmap(self):
        close = self.part("fn toplevel_destroyed(", "fn grab(&mut self")
        minimize = self.part("ControlCommand::Minimize(_) => {", "ControlCommand::Close(_) => {")
        for section in (close, minimize):
            self.assertIn("self.refocus_after_departure(", section)
            self.assertLess(section.index("unmap_elem"), section.index("self.refocus_after_departure("))

    def test_fallback_never_focuses_hidden_or_minimized_windows(self):
        helper = self.part("fn refocus_after_departure(", "fn refresh_start_focus(")
        self.assertIn("self.space.elements().rev().find_map(", helper)
        self.assertIn("!window.minimized", helper)
        self.assertIn("self.workspaces.is_visible(window.id)", helper)
        self.assertIn("keyboard.current_focus().as_ref() != Some(departed)", helper)
        self.assertIn("keyboard.set_focus(self, next.map(", helper)
        self.assertIn("self.active_window = next.as_ref().map(", helper)

    def test_clean_stale_press_and_start_restore_target(self):
        helper = self.part("fn refocus_after_departure(", "fn refresh_start_focus(")
        self.assertIn("self.last_press = None;", helper)
        self.assertIn("self.start_previous_focus = None;", helper)


if __name__ == "__main__":
    unittest.main()
