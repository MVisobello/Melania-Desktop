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
        self.assertIn('body_scroll.set_vexpand(true);',PANEL)
        self.assertIn('right.set_valign(gtk::Align::Fill);',PANEL)
        self.assertIn('footer.append(&query);',PANEL)
        self.assertNotIn('root.append(&query);',PANEL)
        self.assertIn('query.connect_changed',PANEL)
        self.assertIn('search_focus.grab_focus()',PANEL)

    def test_compact_menu_sidebar_and_search(self):
        self.assertIn('],560,565,0,"melania-start",0)',PANEL)
        self.assertIn('right.set_size_request(190,-1);',PANEL)
        self.assertIn('query.set_size_request(300,-1);',PANEL)
        self.assertIn('w.set_margin(Edge::Bottom,6);',PANEL)
        self.assertIn('right.add_css_class("melania-start-shortcuts");',PANEL)

    def test_circular_account_and_icon_only_power_are_functional(self):
        self.assertIn('account.add_css_class("melania-account-orb");',PANEL)
        self.assertIn('classic_control_glyph("account",light_controls)',PANEL)
        self.assertIn('Command::new("melania-settings").arg("account").spawn()',PANEL)
        self.assertIn('profile.append(&account);',PANEL)
        self.assertIn('profile.append(&username_label);',PANEL)
        self.assertIn('footer.append(&power);',PANEL)
        self.assertNotIn('footer.append(&account);',PANEL)
        self.assertNotIn('footer.append(&more);',PANEL)
        self.assertIn('power.set_popover(Some(&popover));',PANEL)
        self.assertIn('classic_control_glyph("power",light_controls)',PANEL)
        self.assertIn('drawing.set_draw_func(',PANEL)
        self.assertNotIn('power.set_icon_name("system-shutdown-symbolic");',PANEL)
        self.assertNotIn('Image::from_icon_name("avatar-default-symbolic")',PANEL)
        self.assertNotIn('Button::with_label("Spegni")',PANEL)
        self.assertIn('let power=gtk::MenuButton::new();',PANEL)
        self.assertIn('session_actions::availability(action)',PANEL)
        self.assertIn('confirm_session_action(&owner,action)',PANEL)
        self.assertIn('session_actions::Action::Lock,',PANEL)
        self.assertIn('session_actions::Action::Suspend,',PANEL)
        self.assertIn('session_actions::Action::Restart,',PANEL)
        self.assertIn('border-radius: 999px;',CSS)
        self.assertIn('button.melania-account-orb',CSS)
        self.assertIn('button.melania-power-icon-button',CSS)

    def test_most_used_then_alphabetic_apps_in_one_scroll(self):
        self.assertIn('let frequent_heading=label("PIÙ USATE"',PANEL)
        self.assertIn('let all_heading=label("TUTTE LE APPLICAZIONI"',PANEL)
        self.assertIn('start_usage::popular()',PANEL)
        self.assertIn('frequent.truncate(5);',PANEL)
        self.assertIn('alphabetical.sort_by(',PANEL)
        self.assertIn('scroll.set_child(Some(&content));',PANEL)
        self.assertIn('scroll.set_max_content_height(302);',PANEL)
        self.assertIn('scroll.set_policy(gtk::PolicyType::Never,gtk::PolicyType::Automatic);',PANEL)

if __name__=="__main__":
    unittest.main(verbosity=2)
