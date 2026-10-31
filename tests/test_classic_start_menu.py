#!/usr/bin/env python3
"""Regression contracts for Melania's real Win7-inspired Start layout.

These source checks complement, never replace, the nested-Wayland visual smoke.
"""
import re
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
PANEL=(ROOT/"melania-panel/src/main.rs").read_text(encoding="utf-8")
CSS=(ROOT/"melania-panel/assets/panel.css").read_text(encoding="utf-8")

class ClassicStartLayout(unittest.TestCase):
    def test_start_is_attached_to_real_taskbar(self):
        self.assertIn('make_surface(app,Layer::Top,&[Edge::Bottom,Edge::Left,Edge::Right],0,52,0,"melania-panel",52)',PANEL)
        self.assertIn('w.set_margin(Edge::Bottom,6);',PANEL)
        self.assertNotIn('w.set_margin(Edge::Bottom,60);',PANEL)
        self.assertIn('melania-start-panel {',CSS)
        self.assertIn('border-radius: 15px 15px 3px 3px;',CSS)

    def test_two_separate_columns_and_compact_left_rows(self):
        self.assertIn('melania-start-program-column',PANEL)
        self.assertIn('melania-start-shortcuts',PANEL)
        self.assertIn('GtkBox::new(Orientation::Horizontal,0)',PANEL)
        self.assertIn('GtkBox::new(Orientation::Vertical,1)',PANEL)
        self.assertIn('icon.set_pixel_size(27);',PANEL)
        self.assertIn('min-height: 29px;',CSS)
        self.assertIn('min-height: 23px;',CSS)
        self.assertIn('border-left: 1px solid #7992ad;',CSS)
        self.assertIn('window.melania-theme-dark',CSS)
        self.assertIn('window.melania-theme-light',CSS)

    def test_classic_system_categories_are_real_actions(self):
        self.assertNotIn('ACCESSI RAPIDI',PANEL)
        for item in ('Documenti','Immagini','Musica','Video','Download',
                     'Computer','Pannello di controllo',
                     'Dispositivi e stampanti','Programmi predefiniti',
                     'Guida e supporto'):
            self.assertIn(f'("{item}",',PANEL)
        for page in ('"system"','"devices"','"applications"'):
            self.assertIn(page,PANEL)
        self.assertIn('Command::new("melania-files")',PANEL)
        self.assertIn('Command::new("melania-settings")',PANEL)
        self.assertIn('Command::new("xdg-open")',PANEL)
        self.assertIn('la gestione completa delle stampanti non è ancora disponibile',PANEL)

    def test_right_icons_monochrome_and_search_at_bottom(self):
        self.assertIn('fn classic_link_row(',PANEL)
        self.assertIn('image.set_pixel_size(16);',PANEL)
        self.assertIn('melania-classic-symbolic',CSS)
        self.assertIn('color: #d7e2ee;',CSS)
        self.assertIn('footer.append(&query);',PANEL)
        self.assertNotIn('root.append(&query);',PANEL)
        self.assertIn('query.connect_changed',PANEL)
        self.assertIn('search_focus.grab_focus()',PANEL)

    def test_recent_apps_stay_on_application_side(self):
        self.assertIn('left.append(&recent_heading);',PANEL)
        self.assertIn('left.append(&recent_list);',PANEL)
        self.assertNotIn('right.append(&recent_list);',PANEL)
        self.assertIn('recent_list.set_visible(count>0);',PANEL)

if __name__=="__main__":
    unittest.main(verbosity=2)
