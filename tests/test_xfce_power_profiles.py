#!/usr/bin/env python3
"""Real Linux power profile selector must use a whitelist and explicit click."""
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[1]
SOURCE=(ROOT/"melania-settings/src/gnome_pages.rs").read_text()
CATALOG=(ROOT/"melania-settings/src/xfce_catalog.rs").read_text()

class PowerProfiles(unittest.TestCase):
    def test_only_supported_modes_and_explicit_action(self):
        self.assertIn('fn parse_power_profiles(output:&str)',SOURCE)
        for value in ('"power-saver"','"balanced"','"performance"'):
            self.assertIn(value,SOURCE)
        self.assertIn('Command::new("powerprofilesctl").arg("list")',SOURCE)
        self.assertIn('read_command("powerprofilesctl",&["get"])',SOURCE)
        self.assertIn('apply.connect_clicked(move |_|',SOURCE)
        self.assertIn('Command::new("powerprofilesctl").args(["set",profile]).status()',SOURCE)
        self.assertIn('I timer di inattività',SOURCE)
        self.assertIn('Stato batteria e selezione profilo energetico',CATALOG)

    def test_unavailable_service_does_not_show_fake_controls(self):
        self.assertIn('if profile_list.is_empty()',SOURCE)
        self.assertIn('power-profiles-daemon non è disponibile',SOURCE)
        self.assertIn('only_whitelisted_power_profiles_are_parsed',SOURCE)
        self.assertNotIn('Command::new("sh")',SOURCE)
        self.assertNotIn('Command::new("bash")',SOURCE)
if __name__=="__main__":
    unittest.main(verbosity=2)
