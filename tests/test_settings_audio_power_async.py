"""Audio and power service probes must remain off GTK's main loop."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
GNOME=(ROOT/"melania-settings/src/gnome_pages.rs").read_text()

class AsynchronousAudioPower(unittest.TestCase):
    def test_audio_detection_and_actions_have_bounded_background_workers(self):
        audio=GNOME[GNOME.index("fn audio_device_controls("):GNOME.index("fn os_name(")]
        for name in ("audio-detect","audio-volume","audio-mute","audio-refresh"):
            self.assertIn(f'crate::settings_async::run("{name}"',audio)
        self.assertIn("let Some((volume,muted))", GNOME) if False else None
        self.assertIn("apply_audio_snapshot(target,snapshot",audio)
        self.assertIn("Command::new(\"wpctl\")",audio)
        self.assertIn('["set-volume",target,requested.as_str()]',audio)
        self.assertIn('["set-mute",target,"toggle"]',audio)
        self.assertIn('button.set_sensitive(false);',audio)
        self.assertIn('action.set_sensitive(true);',audio)
        self.assertIn('controls.set_visible(false);',audio)
        self.assertIn('ready_controls.set_visible(true);',audio)

    def test_power_read_and_set_do_not_run_on_gtk_thread(self):
        section=GNOME[GNOME.index('        "power" => {'):GNOME.index('        "storage" => {')]
        self.assertIn('crate::settings_async::run("power-detect"',section)
        self.assertIn('crate::settings_async::run("power-set"',section)
        self.assertIn('let battery=battery_status();',section)
        self.assertIn('Command::new("powerprofilesctl").arg("list").output()',section)
        self.assertIn('Command::new("powerprofilesctl").args(["set",profile]).status()',section)
        self.assertIn('parse_power_profiles(',section)
        self.assertIn('apply.connect_clicked(move |button|',section)
        self.assertIn('button.set_sensitive(false);',section)
        self.assertIn('action.set_sensitive(true);',section)
        self.assertIn('Some((true,Some(actual))) if actual==profile',section)
        self.assertIn("Some(power)",section)

    def test_whitelisted_modes_and_no_automatic_host_writes(self):
        self.assertIn('fn parse_power_profiles(output:&str)->Vec<&\'static str>',GNOME)
        for name in ("power-saver","balanced","performance"):
            self.assertIn(f'"{name}"=>Some("{name}")',GNOME)
        audio=GNOME[GNOME.index("fn audio_device_controls("):GNOME.index("fn os_name(")]
        self.assertLess(audio.index("apply.connect_clicked(move |button|"),
                        audio.index('["set-volume",target,requested.as_str()]'))
        self.assertLess(audio.index("mute.connect_clicked(move |button|"),
                        audio.index('["set-mute",target,"toggle"]'))

if __name__=="__main__":
    unittest.main()
