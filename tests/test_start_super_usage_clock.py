#!/usr/bin/env python3
"""Start keyboard, local usage and session-menu source-level contracts."""
import unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
PANEL=(ROOT/"melania-panel/src/main.rs").read_text()
COMP=(ROOT/"melania-compositor/src/main.rs").read_text()
HISTORY=(ROOT/"melania-panel/src/start_history.rs").read_text()
USAGE=(ROOT/"melania-panel/src/start_usage.rs").read_text()
SETTINGS=(ROOT/"melania-settings/src/gnome_pages.rs").read_text()
CSS=(ROOT/"melania-panel/assets/panel.css").read_text()

class InteractionContracts(unittest.TestCase):
    def test_one_power_menu_not_two_buttons(self):
        self.assertIn('let power=gtk::MenuButton::new();',PANEL)
        self.assertIn('power.set_popover(Some(&popover));',PANEL)
        self.assertNotIn('let more=gtk::MenuButton::new();',PANEL)
        self.assertNotIn('footer.append(&more);',PANEL)
        self.assertIn('session_actions::Action::Lock,',PANEL)
        self.assertIn('session_actions::Action::Suspend,',PANEL)
        self.assertIn('session_actions::Action::Restart,',PANEL)
        self.assertIn('session_actions::Action::PowerOff,',PANEL)
        self.assertIn('confirm_session_action(&owner,action)',PANEL)

    def test_profile_has_orb_and_larger_label(self):
        self.assertIn('profile.append(&account);',PANEL)
        self.assertIn('profile.append(&username_label);',PANEL)
        self.assertNotIn('footer.append(&account);',PANEL)
        self.assertIn('font-size: 15px;',CSS)

    def test_super_is_release_toggled_not_combo(self):
        for key in ('KEY_Super_L','KEY_Super_R'):
            self.assertIn(key,COMP)
        self.assertIn('super_candidate=false;',COMP)
        self.assertIn('if pressed {super_candidate=false;}',COMP)
        self.assertIn('toggle_start=true;',COMP)
        self.assertIn('socket.send_to(b"TOGGLE_START"',COMP)
        self.assertIn('melania_control::START_SOCKET_ENV',COMP)
        self.assertIn('install_start_hotkey(&start)',PANEL)
        self.assertIn('socket.set_nonblocking(true)',PANEL)
        self.assertIn('fs::Permissions::from_mode(0o600)',PANEL)
        self.assertIn('if owner.is_visible(){owner.set_visible(false);}',PANEL)
        self.assertIn('glib::idle_add_local_once',PANEL)

    def test_usage_stays_private_and_clearable(self):
        self.assertIn('crate::start_usage::record_launch(id)',HISTORY)
        self.assertIn('MAX_ENTRIES:usize=256',USAGE)
        self.assertIn('opts.mode(0o600)',USAGE)
        self.assertIn('Permissions::from_mode(0o700)',USAGE)
        self.assertIn('fs::rename(&tmp,path)',USAGE)
        self.assertIn('melania_config::app_usage_path()',SETTINGS)
        self.assertIn('fs::remove_file(&path)',SETTINGS)
        self.assertIn('frequent.truncate(5);',PANEL)
        self.assertIn('scroll.set_policy(gtk::PolicyType::Never,gtk::PolicyType::Automatic)',PANEL)

    def test_clock_date_left_time_right(self):
        self.assertLess(PANEL.index('clock_row.append(&date_label);'),PANEL.index('clock_row.append(&time_label);'))
        self.assertIn('now.format("%H:%M")',PANEL)
        self.assertIn('now.format("%d/%m/%Y")',PANEL)
        self.assertIn('melania-clock-date',CSS)
        self.assertIn('melania-clock-time',CSS)

if __name__=="__main__":
    unittest.main(verbosity=2)
