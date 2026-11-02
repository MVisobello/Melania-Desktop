"""Regression contracts: Storage/Display hardware probes never block GTK."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
GNOME=(ROOT/"melania-settings/src/gnome_pages.rs").read_text()
BACKLIGHT=(ROOT/"melania-settings/src/display_controls.rs").read_text()
SENSORS=(ROOT/"melania-settings/src/sensors.rs").read_text()
MAIN=(ROOT/"melania-settings/src/main.rs").read_text()
SMOKE=(ROOT/"scripts/smoke-full-desktop-wayland.sh").read_text()

class StorageAndHardwareResponsiveness(unittest.TestCase):
    def test_real_df_runs_only_inside_worker(self):
        storage=GNOME[GNOME.index('        "storage" => {'):GNOME.index('        "bluetooth" => {')]
        self.assertIn('crate::settings_async::run("storage-df",root_storage,move |result|',storage)
        self.assertIn('crate::settings_async::run("storage-refresh",root_storage,move |result|',storage)
        self.assertIn('refresh.connect_clicked(move |button|',storage)
        self.assertIn('refresh.set_halign(Align::Start)',storage)
        self.assertIn('action.set_sensitive(true)',storage)
        self.assertIn('Command::new("df").args(["-h", "--output=size,used,avail,pcent", "/"])',GNOME)

    def test_accelerometer_probe_is_async_and_never_changes_rotation(self):
        section=GNOME[GNOME.index('        "display" => {'):GNOME.index('        "audio" => {')]
        self.assertIn('crate::settings_async::run("display-sensor",sensors::accelerometer',section)
        self.assertIn('crate::settings_async::run("display-sensor-refresh",sensors::accelerometer',section)
        self.assertIn('sensor_refresh.connect_clicked(move |button|',section)
        self.assertIn('sensor_description.set_text("Rilevamento accelerometro in corso…");',section)
        self.assertIn('"HasAccelerometer"',SENSORS)
        self.assertNotIn('Command::new("xrandr")',section)

    def test_brightness_slider_requires_real_backlight_and_worker(self):
        for service in ('backlight-detect','backlight-set','backlight-refresh'):
            self.assertIn(f'crate::settings_async::run("{service}"',BACKLIGHT)
        self.assertIn('controls.set_visible(false)',BACKLIGHT)
        self.assertIn('ready_controls.set_visible(true)',BACKLIGHT)
        self.assertIn('Scale::with_range(Orientation::Horizontal,5.0,100.0,1.0)',BACKLIGHT)
        self.assertIn('["-c","backlight","set",requested.as_str()]',BACKLIGHT)
        self.assertLess(BACKLIGHT.index('apply.connect_clicked(move |button|'),
                        BACKLIGHT.index('["-c","backlight","set",requested.as_str()]'))
        self.assertIn('value.set_tooltip_text',BACKLIGHT)

    def test_bluetooth_controller_description_stays_off_gtk(self):
        section=GNOME[GNOME.index('        "bluetooth" => {'):GNOME.index('        "applications" => {')]
        self.assertIn('crate::settings_async::run("bluetooth-info"',section)
        self.assertIn('read_command("bluetoothctl",&["show"])',section)

    def test_gvfs_smoke_marker_is_one_atomic_pipe_write(self):
        source=(ROOT/"melania-files/src/gio_remote.rs").read_text()
        self.assertIn('let marker=format!("MELANIA_GIO_LISTING_READY count={count}',source)
        self.assertIn('std::io::Write::write_all(&mut std::io::stderr(),marker.as_bytes())',source)
        self.assertNotIn('eprintln!("MELANIA_GIO_LISTING_READY',source)

    def test_actual_nested_wayland_and_probes_covered(self):
        self.assertIn('MELANIA_SETTINGS_STORAGE_DISPLAY_NAVIGATION_OK',MAIN)
        for service in ('storage-df','backlight-detect','display-sensor'):
            self.assertIn(f'service={service} outcome=',SMOKE)
        self.assertIn('MELANIA_SMOKE_SETTINGS_STORAGE_DISPLAY=1',SMOKE)

if __name__=="__main__":
    unittest.main()
