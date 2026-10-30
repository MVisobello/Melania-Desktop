"""Contract tests for GNOME-style Melania Settings and Fluent Office artwork."""
import importlib.util
from pathlib import Path
import sys
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]

class SettingsLayoutContract(unittest.TestCase):
    def test_pages_have_real_navigation_and_read_only_labels(self):
        main = (ROOT / "melania-settings/src/main.rs").read_text()
        pages = (ROOT / "melania-settings/src/gnome_pages.rs").read_text()
        self.assertIn("mod gnome_pages;", main)
        self.assertIn("gnome_pages::overview(&stack)", main)
        self.assertIn("gnome_pages::add_pages(&stack)", main)
        self.assertIn("stack.set_visible_child_name(&target)", pages)
        self.assertIn("command", pages.lower())
        self.assertIn("questa pagina", pages.lower())
        for name in ("display", "audio", "power", "storage", "bluetooth", "applications",
                     "account", "datetime", "devices", "privacy", "region", "about"):
            self.assertIn(f'Some("{name}")', pages)

    def test_window_controls_are_native_actions(self):
        settings = (ROOT / "melania-settings/src/main.rs").read_text()
        shared = (ROOT / "branding/window-chrome-win7.css").read_text()
        files = (ROOT / "melania-files/src/main.rs").read_text()
        for method in ("win.minimize()", "win.maximize()", "win.unmaximize()", "win.close()"):
            self.assertIn(method, settings)
        for name in ("win7-minimize", "win7-maximize", "win7-close"):
            self.assertIn(name, settings)
            self.assertIn(name, shared)
        self.assertIn('include_str!("../../branding/window-chrome-win7.css")', settings)
        self.assertIn('include_str!("../../branding/window-chrome-win7.css")', files)

    def test_office_icon_is_original_not_ms_office(self):
        sys.path.insert(0, str(ROOT / "tools"))
        from melania_fluent_office import OFFICE_CATEGORY_NAMES, office_artwork
        for name in OFFICE_CATEGORY_NAMES:
            for size in (16, 48, 256):
                svg = office_artwork(name, "apps", "light", size)
                self.assertIsNotNone(svg)
                ET.fromstring(svg)
                self.assertNotIn("Microsoft", svg)
                self.assertIn("#178a68", svg)
                self.assertIn("#ef823e", svg)
        self.assertIsNone(office_artwork("libreoffice-writer", "apps", "light", 48))
        builder = (ROOT / "tools/build_melania_fluent_themes.py").read_text()
        self.assertLess(builder.index("office_artwork(name"), builder.index("approved_artwork(name"))

if __name__ == "__main__":
    unittest.main()
