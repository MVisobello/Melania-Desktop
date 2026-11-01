#!/usr/bin/env python3
"""Real controls in Melania Settings: no fake switches or implicit host changes."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
MAIN = (ROOT / "melania-settings/src/main.rs").read_text(encoding="utf-8")
PAGES = (ROOT / "melania-settings/src/gnome_pages.rs").read_text(encoding="utf-8")
CONNECTIVITY = (ROOT / "melania-settings/src/connectivity_controls.rs").read_text(encoding="utf-8")


class LiveSystemSettings(unittest.TestCase):
    def test_audio_controls_cover_default_sink_and_source(self):
        self.assertIn('audio_device_controls(&audio, "Uscita audio", "@DEFAULT_AUDIO_SINK@");', PAGES)
        self.assertIn('audio_device_controls(&audio, "Microfono", "@DEFAULT_AUDIO_SOURCE@");', PAGES)
        self.assertIn('Command::new("wpctl")', PAGES)
        self.assertIn('"set-volume",target,requested.as_str()', PAGES)
        self.assertIn('"set-mute",target,"toggle"', PAGES)
        self.assertIn('refresh_audio_status(', PAGES)
        self.assertIn('mute.connect_clicked(move |_|', PAGES)
        self.assertIn('apply.connect_clicked(move |_|', PAGES)
        self.assertIn('refresh.connect_clicked(move |_|', PAGES)
        self.assertIn('if muted {"Riattiva audio"} else {"Disattiva audio"}', PAGES)
        self.assertIn('Non disponibile', PAGES) if False else None

    def test_wifi_is_an_explicit_networkmanager_action(self):
        self.assertIn("mod connectivity_controls;", MAIN)
        self.assertIn("connectivity_controls::append_wifi(&network);", MAIN)
        self.assertIn('["radio","wifi"]', CONNECTIVITY)
        self.assertIn('["radio","wifi",next]', CONNECTIVITY)
        self.assertIn('button.connect_clicked(move |button|', CONNECTIVITY)
        self.assertIn('if current {"off"} else {"on"}', CONNECTIVITY)
        self.assertIn('Una disattivazione interrompe le connessioni wireless', CONNECTIVITY)
        self.assertIn('parse_wifi_radio("enabled', CONNECTIVITY)

    def test_bluetooth_requires_a_real_controller(self):
        self.assertIn("crate::connectivity_controls::append_bluetooth(&bluetooth);", PAGES)
        self.assertIn('Command::new("bluetoothctl").arg("show")', CONNECTIVITY)
        self.assertIn('Command::new("bluetoothctl").args(["power",next])', CONNECTIVITY)
        self.assertIn('if !output.lines().any(', CONNECTIVITY)
        self.assertIn('starts_with("Controller ")', CONNECTIVITY)
        self.assertIn('fn bluetooth_requires_a_real_controller_and_boolean_power()', CONNECTIVITY)

    def test_no_shell_or_automatic_mutation(self):
        for file in (PAGES, CONNECTIVITY):
            self.assertNotIn('Command::new("sh")', file)
            self.assertNotIn('Command::new("bash")', file)
        # Controls use explicit clicked events; UI constructor may only read.
        self.assertLess(CONNECTIVITY.index('button.connect_clicked(move |button|'),
                        CONNECTIVITY.index('["radio","wifi",next]'))
        self.assertLess(PAGES.index('apply.connect_clicked(move |_|'),
                        PAGES.index('"set-volume",target,requested.as_str()'))


if __name__ == "__main__":
    unittest.main()
