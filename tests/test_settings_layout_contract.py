"""Melania GNOME Settings and Windows 7 chrome integration regression tests."""
import sys
from pathlib import Path
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]

class SettingsUIContract(unittest.TestCase):
    def test_gtk_navigation_and_system_categories(self):
        main = (ROOT / "melania-settings/src/main.rs").read_text()
        pages = (ROOT / "melania-settings/src/gnome_pages.rs").read_text()
        self.assertIn("mod gnome_pages;", main)
        self.assertIn("gnome_pages::overview(&stack)", main)
        self.assertIn("gnome_pages::add_pages(&stack)", main)
        self.assertIn("stack.set_visible_child_name(&target)", pages)
        for name in ("system","network","bluetooth","appearance","notifications",
                     "applications","account","datetime","power","audio","devices","privacy",
                     "region","accessibility","about","desktop","workspaces","shortcuts",
                     "display","storage"):
            self.assertIn(f'"{name}"', main)
        self.assertIn('RefCell::new("system".into())', main)

    def test_real_windows_operations(self):
        main=(ROOT / "melania-settings/src/main.rs").read_text()
        files=(ROOT / "melania-files/src/main.rs").read_text()
        chrome=(ROOT / "branding/window-chrome-win7.css").read_text()
        for call in ("w.minimize()", "w.maximize()", "w.unmaximize()", "w.close()"):
            self.assertIn(call, main)
        for cssclass in ("win7-minimize", "win7-maximize", "win7-close"):
            self.assertIn(cssclass, main)
            self.assertIn(cssclass, chrome)
        self.assertIn('include_str!("../../branding/window-chrome-win7.css")', files)
        self.assertIn('include_str!("../../branding/window-chrome-win7.css")', main)

    def test_office_category_uses_original_suite_icon(self):
        sys.path.insert(0, str(ROOT / "tools"))
        from melania_office_artwork import office_icon
        from melania_fluent_reference import artwork
        for pixels in (16, 48, 128):
            svg=office_icon(pixels)
            ET.fromstring(svg)
            self.assertIn("url(#green)", svg if pixels > 24 else office_icon(128))
        self.assertEqual(artwork("applications-office", "apps", "light", 96),
                         office_icon(96))
        self.assertIsNone(artwork("libreoffice-writer", "apps", "light", 96))

if __name__=="__main__":
    unittest.main()
