#!/usr/bin/env python3
"""Visual design contracts for the real GTK Start panel and original tray icons."""
from __future__ import annotations

from pathlib import Path
import sys
import unittest
from xml.etree import ElementTree as ET

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/"tools"))
from melania_shell_symbolic_icons import TRAY, artwork

DEFAULT_ICONS=(
    "melania-tray-wifi-symbolic",
    "melania-tray-bluetooth-symbolic",
    "melania-tray-volume-symbolic",
    "melania-tray-battery-symbolic",
    "melania-tray-notifications-symbolic",
)


class StartTrayDesignContracts(unittest.TestCase):
    def test_start_menu_has_real_larger_icons_and_search(self):
        panel=(ROOT/"melania-panel/src/main.rs").read_text()
        css=(ROOT/"melania-panel/assets/panel.css").read_text()
        for phrase in ('label("Applicazioni", "melania-start-title")',
                       'heading.add_css_class("melania-start-heading")',
                       'melania-menu-app-icon', 'icon.set_pixel_size(34);',
                       '"org.melaniadesktop.Settings"',
                       '"org.melaniadesktop.Files"',
                       'MELANIA_SMOKE_OPEN_START'):
            self.assertIn(phrase,panel)
        for label in ('melania-start-panel .melania-start-heading',
                      'melania-start-panel .melania-start-title',
                      'melania-start-panel .melania-start-apps .melania-action',
                      'melania-start-panel .melania-start-shortcuts'):
            self.assertIn(label,css)
        self.assertIn('min-height: 51px;',css)
        self.assertIn('window.melania-theme-light .melania-start-panel',css)
        self.assertIn('window.melania-theme-dark .melania-start-panel',css)
        self.assertIn('window.melania-theme-aero .melania-start-panel',css)

    def test_default_tray_icons_are_really_packaged_svg(self):
        panel=(ROOT/"melania-panel/src/main.rs").read_text()
        builder=(ROOT/"tools/build_melania_fluent_themes.py").read_text()
        for icon in DEFAULT_ICONS:
            with self.subTest(icon=icon):
                self.assertIn(icon,TRAY)
                self.assertIn(icon,panel)
                for variant in ("light","dark"):
                    source=artwork(icon,"status",variant,22)
                    self.assertIsNotNone(source)
                    ET.fromstring(source)
                    self.assertIn('viewBox="0 0 24 24"',source)
                    self.assertNotIn('currentColor',source)
                    self.assertNotIn('linearGradient',source)
                    self.assertNotIn('font',source)
                    self.assertGreaterEqual(source.count("<path")+source.count("<rect")+source.count("<circle"),2)
        self.assertIn('unique.add(f"status/{size}/{name}.svg")',builder)
        self.assertIn('or approved_artwork(',builder)

    def test_status_icons_22px_with_correct_semantics(self):
        panel=(ROOT/"melania-panel/src/main.rs").read_text()
        css=(ROOT/"melania-panel/assets/panel.css").read_text()
        self.assertIn('icon.set_pixel_size(22);',panel)
        self.assertIn('notice_icon.set_pixel_size(22);',panel)
        for cls in ('melania-status-wifi','melania-status-bluetooth',
                    'melania-status-volume','melania-status-battery'):
            self.assertIn(cls,panel)
        self.assertIn('.melania-bar .melania-tray-button .melania-tray-symbol',css)
        self.assertIn('min-width: 22px;',css)

    def test_qa_shows_actual_start_only_in_ci(self):
        shell=(ROOT/"scripts/smoke-full-desktop-wayland.sh").read_text()
        panel=(ROOT/"melania-panel/src/main.rs").read_text()
        self.assertIn('MELANIA_SMOKE_OPEN_START=1 target/debug/melania-compositor',shell)
        self.assertIn('start.present();',panel)
        self.assertIn('std::env::var_os("MELANIA_SMOKE_OPEN_START")',panel)


if __name__=="__main__":
    unittest.main(verbosity=2)
