"""Regression checks for flush Windows 7 window controls in Melania GTK apps.

These source-level guards complement the existing GTK/Wayland screenshot smoke
test. Only header spacing changes: the three button glyphs, shape and actions
remain governed by the shared component.
"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
CHROME = (ROOT / "branding/window-chrome-win7.css").read_text()
CONTROLS = (ROOT / "branding/window_controls.rs").read_text()


class WindowControlsFlushEdges(unittest.TestCase):
    def test_shared_component_used_by_both_main_window_types(self):
        for path, header in (
            ("melania-files/src/main.rs", "aero_header"),
            ("melania-settings/src/main.rs", "titlebar"),
        ):
            with self.subTest(path=path):
                source = (ROOT / path).read_text()
                self.assertIn(f"window_controls::install(&window, &{header});", source)
                self.assertIn(
                    'include_str!("../../branding/window-chrome-win7.css")',
                    source,
                )
        self.assertIn("controls.set_valign(gtk::Align::Start);", CONTROLS)

    def test_aero_compensates_frame_border_and_end_padding(self):
        self.assertIn("padding: 0 8px 0 10px;", CHROME)
        self.assertIn("border: 1px solid #c6e9ff;", CHROME)
        self.assertRegex(
            CHROME,
            r"headerbar\.melania-win7-header \.win7-window-controls\s*"
            r"\{[^}]*margin: -1px -9px 0 8px;",
        )

    def test_borderless_light_dark_are_exactly_flush(self):
        self.assertIn("headerbar.melania-win7-header.melania-theme-light .win7-window-controls", CHROME)
        self.assertIn("headerbar.melania-win7-header.melania-theme-dark .win7-window-controls", CHROME)
        self.assertRegex(
            CHROME,
            r"headerbar\.melania-win7-header\.melania-theme-dark \.win7-window-controls\s*"
            r"\{\s*margin-top: 0;\s*margin-right: -8px;",
        )
        for theme in ("light", "dark"):
            self.assertIn(f"headerbar.melania-win7-header.melania-theme-{theme}:backdrop", CHROME)

    def test_button_appearance_and_window_actions_remain_shared(self):
        for marker in (
            'Button::with_label("—")',
            'Button::with_label("□")',
            'Button::with_label("×")',
            "w.minimize()", "w.maximize()", "w.unmaximize()", "w.close()",
        ):
            self.assertIn(marker, CONTROLS)
        for marker in (
            "min-width: 28px;", "min-height: 18px;",
            "border-radius: 0 0 0 6px;",
            "min-width: 36px;", "border-radius: 0 0 6px 0;",
            "background: linear-gradient(to bottom,#f9b4aa 0%,#d34836 56%,#ab241d 100%);",
        ):
            self.assertIn(marker, CHROME)


if __name__ == "__main__":
    unittest.main()
