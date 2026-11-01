#!/usr/bin/env python3
"""Screen brightness is real, opt-in and does not alter another desktop session."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
DISPLAY=(ROOT/"melania-settings/src/display_controls.rs").read_text()
PAGES=(ROOT/"melania-settings/src/gnome_pages.rs").read_text()
MAIN=(ROOT/"melania-settings/src/main.rs").read_text()

class ScreenBrightness(unittest.TestCase):
    def test_control_is_reachable_and_never_faked(self):
        self.assertIn("mod display_controls;",MAIN)
        self.assertIn("crate::display_controls::append_backlight(&display);",PAGES)
        self.assertIn('let Some(percent)=backlight_percent() else {',DISPLAY)
        self.assertIn('"settings-row-description"',DISPLAY)

    def test_uses_backlight_class_only_on_explicit_click(self):
        self.assertIn('["-c","backlight",operation]',DISPLAY)
        self.assertIn('["-c","backlight","set",requested.as_str()]',DISPLAY)
        self.assertIn('apply.connect_clicked(move |_|',DISPLAY)
        self.assertLess(DISPLAY.index('apply.connect_clicked(move |_|'),
                        DISPLAY.index('["-c","backlight","set",requested.as_str()]'))
        self.assertIn('Scale::with_range(Orientation::Horizontal,5.0,100.0,1.0)',DISPLAY)
        self.assertIn('parse_percent(1,0),None',DISPLAY)
        self.assertNotIn('Command::new("sh")',DISPLAY)
        self.assertNotIn('Command::new("bash")',DISPLAY)

if __name__=="__main__":
    unittest.main()
