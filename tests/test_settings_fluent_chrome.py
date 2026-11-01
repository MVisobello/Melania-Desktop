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
        self.assertIn("melania_config::save(&latest)", source)
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

    def test_system_icons_are_real_svg_pictograms_not_letter_tiles(self):
        from melania_fluent_settings_icons import artwork as system_icon
        names = ("bluetooth", "preferences-desktop-theme",
                 "preferences-system-notifications", "applications-other",
                 "avatar-default", "x-office-calendar", "battery",
                 "audio-card", "input-keyboard", "dialog-information")
        for name in names:
            with self.subTest(icon=name):
                svg = system_icon(name, "apps", "light", 24)
                self.assertIsNotNone(svg)
                root = ET.fromstring(svg)
                self.assertTrue(root.tag.endswith("svg"))
                self.assertNotIn("<text", svg)
                self.assertTrue(any(tag in svg for tag in ("<path", "<rect", "<circle")))
        builder=(ROOT/"tools/build_melania_fluent_themes.py").read_text()
        self.assertIn("settings_artwork(name,category", builder)

    def test_official_app_ids_resolve_without_missing_image_placeholders(self):
        from melania_fluent_artwork import CORE
        from melania_fluent_settings_icons import artwork as system_icon
        ids = ("melania", "org.melaniadesktop.Files", "org.melaniadesktop.Settings")
        for name in ids:
            self.assertTrue(any(icon == name and context == "apps" for _, icon, context in CORE))
        moon = system_icon("melania", "apps", "light", 32)
        self.assertIn('mask="url(#cut)"', moon)
        ET.fromstring(moon)
        build = (ROOT / "tools/build_melania_fluent_themes.py").read_text()
        self.assertIn('unique.add(f"apps/scalable/{app_name}.svg")', build)

    def test_real_window_buttons_styled_in_both_apps(self):
        # Files and Settings must literally install the same Rust component.
        files = (ROOT / "melania-files/src/main.rs").read_text()
        settings = (ROOT / "melania-settings/src/main.rs").read_text()
        controls = (ROOT / "branding/window_controls.rs").read_text()
        chrome = (ROOT / "branding/window-chrome-win7.css").read_text()
        for code, header in ((files, "&aero_header"), (settings, "&titlebar")):
            self.assertIn('mod window_controls;', code)
            self.assertIn('window_controls::install(&window, '+header+');', code)
            self.assertIn('set_titlebar(Some(', code)
            self.assertNotIn('set_show_title_buttons(true)', code)
            self.assertNotIn('set_decoration_layout(Some(":minimize,maximize,close"))', code)
            self.assertIn('include_str!("../../branding/window-chrome-win7.css")', code)
        for name in ("win7-minimize", "win7-maximize", "win7-close",
                     "win7-window-controls", "melania-win7-header"):
            self.assertIn(name, controls)
            self.assertIn(name, chrome)
        for operation in ("w.minimize()", "w.maximize()", "w.unmaximize()", "w.close()"):
            self.assertIn(operation, controls)
        self.assertIn('header.set_show_title_buttons(false)', controls)
        self.assertIn('header.set_decoration_layout(Some(""))', controls)
        self.assertIn("linear-gradient", chrome)
        self.assertIn(":backdrop", chrome)
        for app in ("melania-files", "melania-settings"):
            css = (ROOT / app / "assets/style.css").read_text()
            self.assertNotIn("windowcontrols button", css)
            self.assertNotIn("GtkDialog.fake", css)

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
