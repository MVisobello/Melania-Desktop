"""Source-level regression contracts; do not substitute for cargo or GTK smoke."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]

class StartSessionPrivacyContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.panel=(ROOT/"melania-panel/src/main.rs").read_text()
        cls.history=(ROOT/"melania-panel/src/start_history.rs").read_text()
        cls.session=(ROOT/"melania-panel/src/session_actions.rs").read_text()
        cls.config=(ROOT/"melania-config/src/lib.rs").read_text()
        cls.settings=(ROOT/"melania-settings/src/gnome_pages.rs").read_text()
        cls.sensors=(ROOT/"melania-settings/src/sensors.rs").read_text()

    def test_no_unwanted_start_header(self):
        self.assertNotIn("Il tuo spazio Melania",self.panel)
        self.assertNotIn('root.append(&heading);',self.panel)
        self.assertIn('query.connect_changed',self.panel)
        self.assertIn('gdk::Key::Escape',self.panel)
        self.assertIn('search_focus.grab_focus()',self.panel)

    def test_recent_apps_only_successful_launches_and_no_commands_persisted(self):
        self.assertIn('start_history::record_launch(id);',self.panel)
        self.assertIn('start_history::recent()',self.panel)
        self.assertIn('if let Some(id)=history_id.as_deref()',self.panel)
        self.assertIn('fn valid_id(id: &str)',self.history)
        self.assertIn('create_dir_all(parent)',self.history)
        self.assertIn('LIMIT: usize = 8',self.history)
        self.assertIn('fs::rename(&temp,path)',self.history)
        self.assertIn('pub fn recent_apps_path()',self.config)

    def test_privacy_page_deletes_exact_recent_app_file_only(self):
        self.assertIn('melania_config::recent_apps_path()',self.settings)
        self.assertIn('fs::remove_file(&path)',self.settings)
        self.assertIn('Cancella app recenti',self.settings)
        self.assertNotIn('remove_dir_all',self.settings)

    def test_no_host_shutdown_in_nested_preview(self):
        self.assertIn('MELANIA_NESTED_SESSION',self.session)
        self.assertIn('MELANIA_ALLOW_HOST_POWER_ACTIONS',self.session)
        self.assertIn('availability(action)?;',self.session)
        self.assertIn('ControlCommand::Quit',self.session)
        self.assertIn('Confermi questa operazione?',self.panel)

    def test_rotation_is_discovery_only_until_backend_exists(self):
        self.assertIn('net.hadess.SensorProxy',self.sensors)
        self.assertIn('HasAccelerometer',self.sensors)
        self.assertIn('parse_busctl_boolean',self.sensors)
        self.assertIn('sensors::accelerometer()',self.settings)
        self.assertIn('Rotazione manuale e blocco',self.settings)
        self.assertNotIn('systemctl',self.sensors)

if __name__=="__main__":
    unittest.main()
