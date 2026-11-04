"""Real XFCE feature substitutes are exposed only in isolated Melania login."""
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[1]
H=(ROOT/"melania-settings/src/system_integrations.rs").read_text()
MAIN=(ROOT/"melania-settings/src/main.rs").read_text()
PAGES=(ROOT/"melania-settings/src/gnome_pages.rs").read_text()

class HardwareBridges(unittest.TestCase):
    def test_every_essential_backend_is_an_explicit_rust_whitelist_entry(self):
        self.assertIn('mod system_integrations;',MAIN)
        for binary in ("wdisplays","pavucontrol","nm-connection-editor","blueman-manager",
                       "system-config-printer"):
            self.assertIn(f'=>"{binary}"',H)
        self.assertIn("Command::new(command).spawn()",H)
        self.assertNotIn("sh -c",H)
        self.assertNotIn("bash -c",H)

    def test_embedded_ui_exposes_real_controls_not_a_fake_switch(self):
        for expression in (
            'Tool::Displays','Tool::AudioMixer','Tool::BluetoothDevices','Tool::Printers'
        ):
            self.assertIn(f'crate::system_integrations::{expression}',PAGES)
        self.assertIn('system_integrations::Tool::NetworkProfiles',MAIN)
        self.assertIn('button.set_sensitive(allowed && installed);',H)
        self.assertIn('button.connect_clicked(move |_|',H)
        self.assertIn('row.append(&feedback);',H)

    def test_nested_cinnamon_remains_unmodified(self):
        self.assertIn('MELANIA_EXTERNAL_COMPOSITOR',H)
        self.assertIn('MELANIA_NESTED_SESSION',H)
        self.assertIn('MELANIA_ALLOW_HOST_POWER_ACTIONS',H)
        self.assertIn('XDG_SESSION_TYPE',H)
        self.assertIn("configurazione viene applicata dal servizio Linux",H)
        self.assertNotIn("sudo ",H)

if __name__=="__main__": unittest.main()
