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
        self.assertIn('mapped_search.grab_focus()',PANEL)

    def test_compact_menu_sidebar_and_search(self):
        self.assertIn('],510,565,0,"melania-start",0)',PANEL)
        self.assertIn('right.set_size_request(158,-1);',PANEL)
        self.assertIn('query.set_size_request(250,-1);',PANEL)
        self.assertIn('w.set_margin(Edge::Bottom,6);',PANEL)
        self.assertIn('right.add_css_class("melania-start-shortcuts");',PANEL)

    def test_circular_account_and_icon_only_power_are_functional(self):
        self.assertIn('account.add_css_class("melania-account-orb");',PANEL)
        self.assertIn('classic_control_glyph("account",light_controls)',PANEL)
        self.assertIn('Command::new("melania-settings").arg("account").spawn()',PANEL)
        self.assertIn('profile.append(&account);',PANEL)
        self.assertNotIn('footer.append(&account);',PANEL)
        self.assertNotIn('footer.append(&power);',PANEL)
        self.assertLess(PANEL.index('profile.append(&account);'), PANEL.index('let footer=GtkBox::new'))
        self.assertIn('classic_control_glyph("power",light_controls)',PANEL)
        self.assertIn('drawing.set_draw_func(',PANEL)
        self.assertNotIn('power.set_icon_name("system-shutdown-symbolic");',PANEL)
        self.assertNotIn('Image::from_icon_name("avatar-default-symbolic")',PANEL)
        self.assertNotIn('Button::with_label("Spegni")',PANEL)
        self.assertIn('for action in session_actions::Action::ALL',PANEL)
        self.assertIn('choice.connect_clicked(move |_| confirm_session_action(&owner,action))',PANEL)
        self.assertIn('more.set_child(Some(&classic_control_glyph("power",light_controls)))',PANEL)
        self.assertIn('footer.append(&more);',PANEL)
        self.assertNotIn('power.connect_clicked',PANEL)
        self.assertIn('border-radius: 999px;',CSS)
        self.assertIn('button.melania-account-orb',CSS)
        self.assertIn('menubutton.melania-power-icon-button > button',CSS)

    def test_compact_footer_height_and_alignment(self):
        self.assertIn('query.set_valign(gtk::Align::Center);',PANEL)
        self.assertIn('more.set_valign(gtk::Align::Center);',PANEL)
        self.assertRegex(CSS,r'(?s)\.melania-start-panel \.melania-start-footer \.melania-entry \{\s*min-height: 28px;\s*padding: 2px 9px;')
        self.assertRegex(CSS,r'(?s)\.melania-start-panel \.melania-start-footer menubutton\.melania-power-icon-button > button \{\s*min-width: 34px;\s*min-height: 28px;\s*padding: 2px;')

    def test_start_footer_is_visually_centered_in_fixed_popup(self):
        # GTK reserves extra space below the controls; shift them into visual center.
        self.assertRegex(CSS, r'(?s)\.melania-start-panel \.melania-start-footer \{\s*padding: 21px 3px 0;')
        self.assertIn('query.set_valign(gtk::Align::Center);', PANEL)
        self.assertIn('more.set_valign(gtk::Align::Center);', PANEL)

    def test_keyboard_toggle_and_instant_search_are_wired_to_compositor(self):
        COMPOSITOR=(ROOT/"melania-compositor/src/main.rs").read_text(encoding="utf-8")
        CONTROL=(ROOT/"melania-control/src/lib.rs").read_text(encoding="utf-8")
        self.assertIn('w.set_keyboard_mode(KeyboardMode::Exclusive)',PANEL)
        self.assertIn('w.connect_map(move |_| { let _=mapped_search.grab_focus(); });',PANEL)
        self.assertIn('glib::idle_add_local_once',PANEL)
        self.assertIn('install_start_hotkey_listener(app,&start,&quick,&notifications);',PANEL)
        self.assertIn('socket.recv(&mut packet)',PANEL)
        self.assertIn('b"TOGGLE_START"',PANEL)
        self.assertIn('KEY_Super_L',COMPOSITOR)
        self.assertIn('KEY_Super_R',COMPOSITOR)
        self.assertIn('super_pressed_alone',COMPOSITOR)
        self.assertIn('state.refresh_start_focus();',COMPOSITOR)
        self.assertIn('socket.send_to(b"TOGGLE_START",&destination)',COMPOSITOR)
        self.assertIn('pub fn start_toggle_socket_path(control: &Path)',CONTROL)
        self.assertIn('const START_HIDE_OFFSET: i32 = -4000;',PANEL)
        self.assertIn('w.set_margin(Edge::Left,START_HIDE_OFFSET);',PANEL)
        self.assertIn('w.set_opacity(0.0);',PANEL)
        self.assertIn('w.set_opacity(1.0);',PANEL)
        self.assertIn('w.has_css_class("melania-start-concealed")',PANEL)
        self.assertIn('if geometry.loc.x+geometry.size.w<=0',COMPOSITOR)
        self.assertIn('hide_start_surface(&sm);',PANEL)
        self.assertNotIn('menu.set_visible(false);',PANEL[PANEL.index('fn start_window('):PANEL.index('fn binary(')])

    def test_power_menu_only_in_footer_and_sidebar_uses_height(self):
        self.assertIn('button.set_vexpand(true);',PANEL)
        self.assertIn('let more=gtk::MenuButton::new();',PANEL)
        self.assertIn('more.set_popover(Some(&popover));',PANEL)
        self.assertIn('right.append(&profile);',PANEL)
        self.assertIn('drawing.set_content_width(size);',PANEL)
        self.assertIn('let size=if kind=="account" {27} else {22};',PANEL)
        self.assertIn('min-width: 46px;',CSS)
        self.assertNotIn('let power=Button::new();',PANEL)

    def test_recent_apps_stay_on_application_side(self):
        self.assertIn('left.append(&recent_heading);',PANEL)
        self.assertIn('left.append(&recent_list);',PANEL)
        self.assertNotIn('right.append(&recent_list);',PANEL)
        self.assertIn('recent_list.set_visible(count>0);',PANEL)

if __name__=="__main__":
    unittest.main(verbosity=2)
