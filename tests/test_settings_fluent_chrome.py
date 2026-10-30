#!/usr/bin/env python3
"""Smoke contract for the GNOME-inspired Settings and Windows 7 chrome.

These checks complement, never replace, GTK compilation and visual inspection.
"""
from __future__ import annotations
import sys
import unittest
from pathlib import Path
from xml.etree import ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))
from melania_office_artwork import office_icon
from melania_fluent_reference import artwork as approved
from melania_fluent_artwork import artwork as fluent

class MelaniaDesignContract(unittest.TestCase):
    def test_settings_keeps_working_controls_and_real_navigation(self):
        source = (ROOT / "melania-settings/src/main.rs").read_text()
        self.assertIn('let navigation_scroll = gtk::ScrolledWindow::new()', source)
        self.assertIn('let navigation_list = GtkBox::new(Orientation::Vertical, 1)', source)
        self.assertIn('stack.connect_visible_child_name_notify', source)
        self.assertIn('category_search.connect_changed', source)
        self.assertIn('saved_option(', source)
        self.assertIn('melania_config::save(&preferences)', source)
        for category in ("appearance","desktop","workspaces","network",
                         "accessibility","shortcuts","system"):
            self.assertIn(f'"{category}"', source)
        for css_class in ("settings-navigation-list", "settings-section-heading",
                          "settings-row-title", "settings-row-description"):
            self.assertIn(css_class, source)

    def test_fluent_icons_and_compact_gnome_preferences(self):
        icons = (ROOT / "melania-settings/src/symbols.rs").read_text()
        pages = (ROOT / "melania-settings/src/gnome_pages.rs").read_text()
        css = (ROOT / "melania-settings/assets/gnome-overview.css").read_text()
        smoke = (ROOT / ".github/workflows/wayland-smoke.yml").read_text()
        config = (ROOT / "melania-config/src/lib.rs").read_text()
        self.assertIn('Image::from_icon_name(name)', icons)
        self.assertNotIn('matches!(name, "computer"', icons)
        self.assertIn('hardware.add_css_class("settings-group-card")', pages)
        self.assertIn('software.add_css_class("settings-group-card")', pages)
        self.assertIn('border-bottom: 1px solid #e8edf3', css)
        self.assertIn('build_melania_fluent_themes.py', smoke)
        self.assertIn('install-melania-fluent-icons.sh', smoke)
        self.assertIn('XDG_DATA_HOME: ${{ runner.temp }}/melania-data', smoke)
        self.assertIn('else {"Adwaita"}', config)

    def test_real_window_buttons_styled_in_both_apps(self):
        for app, header in (("melania-files","files-aero-header"),
                            ("melania-settings","settings-titlebar")):
            css=(ROOT / app / "assets/style.css").read_text()
            self.assertIn(f"headerbar.{header} windowcontrols button.minimize", css)
            self.assertIn(f"headerbar.{header} windowcontrols button.close", css)
            self.assertIn("linear-gradient", css)
            self.assertNotIn("GtkDialog.fake", css)
        files=(ROOT / "melania-files/src/main.rs").read_text()
        self.assertIn('set_decoration_layout(Some(":minimize,maximize,close"))',files)
        self.assertIn('set_titlebar(Some(', files)
        settings=(ROOT / "melania-settings/src/main.rs").read_text()
        self.assertIn('set_show_title_buttons(false)', settings)
        self.assertIn('set_titlebar(Some(', settings)
        for operation in ("w.minimize()", "w.maximize()", "w.unmaximize()", "w.close()"):
            self.assertIn(operation, settings)
        chrome=(ROOT / "branding/window-chrome-win7.css").read_text()
        for name in ("win7-minimize", "win7-maximize", "win7-close"):
            self.assertIn(name, chrome)
            self.assertIn(name, settings)

    def test_office_icon_is_generic_original_and_single_exception(self):
        for size in (16,24,32,48,96,128,256):
            xml=office_icon(size)
            root=ET.fromstring(xml)
            self.assertTrue(root.tag.endswith("svg"))
            for trademark in ("microsoft","office.com","Microsoft", "Office 365"):
                self.assertNotIn(trademark, xml)
            self.assertIn("#",xml)
            approved_icon=approved("applications-office","apps","light",size)
            self.assertEqual(xml,approved_icon)
            self.assertEqual(xml,fluent("applications-office","apps","light",size))
        self.assertNotIn("data:image/png;base64",approved("applications-office","apps","light",96))
        self.assertIn("data:image/png;base64",approved("web-browser","apps","light",96))
        self.assertIn("data:image/png;base64",approved("folder-music","places","light",96))

if __name__ == "__main__":
    unittest.main(verbosity=2)
