#!/usr/bin/env python3
"""Regressions for Melania's three UI themes, tab rail and small shell icons."""
from __future__ import annotations
import sys
import unittest
from pathlib import Path
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))
from melania_shell_symbolic_icons import SIDEBAR, TRAY, WIDGETS, artwork


class MelaniaAppearanceAndIconContracts(unittest.TestCase):
    def test_three_exclusive_appearance_options_are_persistent(self):
        cfg = (ROOT / "melania-config/src/lib.rs").read_text()
        settings = (ROOT / "melania-settings/src/main.rs").read_text()
        for theme in ("light", "dark", "aero"):
            self.assertIn(f'"{theme}"', cfg)
            self.assertIn(f'"{theme}"', settings)
        self.assertIn("fn appearance_theme(&self)", cfg)
        self.assertIn("set_appearance_theme(&mut self", cfg)
        self.assertIn("appearance_picker.connect_selected_notify", settings)
        self.assertIn("save_edit(&prefs_for_theme,", settings)
        self.assertNotIn('saved_option("Tema scuro"', settings)
        self.assertNotIn('saved_option("Aero Glass"', settings)

    def test_plus_follows_tabs_and_ctrl_t_is_preserved(self):
        files = (ROOT / "melania-files/src/main.rs").read_text()
        first = files.index('let tab_rail = GtkBox::new(')
        self.assertLess(files.index('tab_rail.append(&tab_scroll)', first),
                        files.index('tab_rail.append(&add_button)', first))
        self.assertNotIn('aero_header.pack_start(&add_button)', files)
        self.assertIn('keys_add.emit_clicked()', files)
        self.assertIn('aero_header.pack_start(&tab_rail)', files)

    def test_shared_window_buttons_are_visibly_smaller(self):
        chrome = (ROOT / "branding/window-chrome-win7.css").read_text()
        self.assertIn('min-width: 28px', chrome)
        self.assertIn('min-height: 18px', chrome)
        self.assertIn('min-width: 36px', chrome)
        self.assertIn('melania-theme-light', chrome)
        self.assertIn('melania-theme-dark', chrome)

    def test_exact_sidebar_and_tray_symbols_are_simple_svg(self):
        files = (ROOT / "melania-files/src/main.rs").read_text()
        panel = (ROOT / "melania-panel/src/main.rs").read_text()
        builder = (ROOT / "tools/build_melania_fluent_themes.py").read_text()
        self.assertIn("shell_artwork(name,category", builder)
        for name in SIDEBAR:
            self.assertIn(name, files) if name != "user-bookmarks-symbolic" else None
            for variant in ("light", "dark"):
                xml = artwork(name, "places", variant, 24)
                ET.fromstring(xml)
                self.assertNotIn("<text", xml)
                self.assertNotIn("linearGradient", xml)
        for name in TRAY:
            # The legacy freedesktop names stay packaged for compatibility.
            # Only the five new Melania tray IDs are loaded by the panel.
            if name.startswith("melania-tray-") and not name.endswith("-light"):
                self.assertIn(name, panel)
            for variant in ("light", "dark"):
                xml = artwork(name, "status", variant, 20)
                ET.fromstring(xml)
                self.assertNotIn("linearGradient", xml)
        self.assertIn("unique.add(f\"places/{size}/{name}.svg\")", builder)
        self.assertIn("unique.add(f\"status/{size}/{name}.svg\")", builder)
        for name in WIDGETS:
            svg = artwork(name, "actions", "light", 24)
            ET.fromstring(svg)
            self.assertNotIn("linearGradient", svg)
        self.assertIn('unique.add(f"actions/{size}/{name}.svg")', builder)
        self.assertNotIn('("⌂  Home", home())', files)

    def test_theme_is_used_by_files_and_panel_not_only_settings(self):
        files = (ROOT / "melania-files/src/main.rs").read_text()
        panel = (ROOT / "melania-panel/src/main.rs").read_text()
        for src in (files, panel):
            self.assertIn("appearance_theme()", src)
            self.assertIn("gtk-application-prefer-dark-theme", src)
        for css_file in ("melania-files/assets/style.css", "melania-panel/assets/panel.css"):
            css = (ROOT / css_file).read_text()
            self.assertIn("melania-theme-light", css)
            self.assertIn("melania-theme-dark", css)


if __name__ == "__main__":
    unittest.main(verbosity=2)
